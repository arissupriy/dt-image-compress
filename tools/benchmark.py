#!/usr/bin/env python3
"""Reproduce synthetic smoke tests or a local JPEG sample comparison."""
import argparse
import csv
import hashlib
import io
import json
import math
import os
from pathlib import Path
import platform
import statistics
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def summarize(rows):
    groups = {}
    for row in rows:
        status = row.get("status", "ok")
        assert status in ("ok", "unreachable")
        if status == "ok":
            assert int(row["output_bytes"]) > 0
            if int(row["target_bytes"]):
                assert int(row["output_bytes"]) <= int(row["target_bytes"])
        else:
            assert int(row["best_observed_bytes"]) > int(row["target_bytes"])
        groups.setdefault((row["sample"], row["method"], row["target_bytes"], status), []).append(row)
    result = []
    for (sample, method, target, status), items in groups.items():
        times = sorted(float(r["elapsed_ms"]) for r in items)
        result.append({"sample": sample, "method": method, "runs": len(items),
                       "status": status,
                       "output_bytes": int(items[0]["output_bytes"]) if status == "ok" else None,
                       "target_bytes": int(target),
                       "median_ms": statistics.median(times),
                       "p95_ms": times[max(0, math.ceil(0.95 * len(times)) - 1)],
                       "psnr_rgb": items[0]["psnr_rgb"],
                       "text_pixel_psnr": items[0]["text_pixel_psnr"],
                       "quality": items[0]["quality"],
                       "width": items[0].get("width"), "height": items[0].get("height"),
                       "resized": items[0].get("resized"),
                       "best_observed_bytes": int(items[0]["best_observed_bytes"]) if status != "ok" else None})
    return result


def self_test():
    row = {"sample": "fixture", "method": "test", "elapsed_ms": "2",
           "output_bytes": "10", "target_bytes": "10", "psnr_rgb": "40",
           "text_pixel_psnr": "nan", "quality": "80"}
    assert summarize([row])[0]["median_ms"] == 2
    assert len(summarize([row, {**row, "target_bytes": "20"}])) == 2
    unreachable = {**row, "status": "unreachable", "output_bytes": "",
                   "best_observed_bytes": "15", "quality": "", "psnr_rgb": "nan"}
    assert summarize([unreachable])[0]["best_observed_bytes"] == 15
    try:
        summarize([{**row, "output_bytes": "11"}])
    except AssertionError:
        pass
    else:
        raise AssertionError("oversized success was accepted")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--iterations", type=int, default=5)
    parser.add_argument("--output", type=Path, default=ROOT / "benchmark-results")
    parser.add_argument("--samples", type=Path, help="local JPEG directory; originals are never copied")
    suite = parser.add_mutually_exclusive_group()
    suite.add_argument("--jpeg-suite", action="store_true", help="200KB JPEG backend/filter ablation with desktop resource limits")
    suite.add_argument("--jpeg-native-suite", action="store_true", help="200KB Turbo/MozJPEG/jpegli, DQT, log-secant and hybrid ablation (Linux, Rust >=1.90)")
    suite.add_argument("--jpeg-chroma-suite", action="store_true", help="200KB Turbo/jpegli444/420 without prefilter (Linux, Rust >=1.90)")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    self_test()
    if args.self_test:
        print("benchmark self-check passed")
        return
    if args.iterations < 1:
        parser.error("iterations must be positive")
    native_build = args.jpeg_native_suite or args.jpeg_chroma_suite
    if native_build:
        args.jpeg_suite = True
    if args.jpeg_suite and not args.samples:
        parser.error("--jpeg-suite requires --samples")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    features = "experimental-prefilter,mozjpeg,jpegli" if native_build else "experimental-prefilter"
    if args.jpeg_chroma_suite:
        features = "cli," + features
    run_env = os.environ.copy()
    if native_build:
        run_env.update(TURBOJPEG_DYNAMIC="1", TURBOJPEG_STATIC="0", TURBOJPEG_SOURCE="vendor")
    cargo = ["cargo", "--locked", "--release", "--example", "benchmark", "--features", features]
    subprocess.run([cargo[0], "build", *cargo[1:]], cwd=ROOT, env=run_env, check=True)
    # Cargo supplies the bundled native library search path for the shared build.
    command = ([cargo[0], "run", *cargo[1:], "--"] if native_build else
               [str(ROOT / "target/release/examples/benchmark")])
    command.extend([str(args.iterations), str(output)])
    samples = sorted(p for p in args.samples.resolve().iterdir()
                     if p.is_file() and p.suffix.lower() in (".jpg", ".jpeg")) if args.samples else []
    if args.samples:
        if not samples:
            parser.error("no JPEG samples found")
        command.append(str(args.samples.resolve()))
        if args.jpeg_suite:
            command.append("jpeg-chroma-suite" if args.jpeg_chroma_suite else
                           ("jpeg-native-suite" if args.jpeg_native_suite else "jpeg-suite"))
    run = subprocess.run(command, cwd=ROOT, env=run_env, check=True, stdout=subprocess.PIPE, text=True)
    (output / "results.csv").write_text(run.stdout)
    summary = summarize(list(csv.DictReader(io.StringIO(run.stdout))))
    (output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    manifest = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in output.glob("*-input.png")}
    if args.samples:
        manifest = {f"sample-{i:02}": {"filename": p.name, "bytes": p.stat().st_size,
                    "sha256": hashlib.sha256(p.read_bytes()).hexdigest()}
                    for i, p in enumerate(samples, 1)}
    environment = {"platform": platform.platform(), "python": platform.python_version(),
                   "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
                   "iterations": args.iterations, "warmups_per_jpeg_method": 0 if args.jpeg_suite else 1,
                   "dataset": "local user JPEG samples; not a representative corpus" if args.samples else "five synthetic smoke fixtures; not a research evaluation corpus",
                   "rss_scope": "whole benchmark process, excluding compilation; not per image",
                   "rss_measured": (output / "peak-rss-kib.txt").is_file(),
                   "rss_source": "Linux /proc/self/status VmHWM when available",
                   "jpeg_encoder": ("Turbo444/420, Google jpegli444/420 (native AQ enabled); no prefilter"
                                    if args.jpeg_chroma_suite else "Turbo444, MozJPEG444, MozJPEG444 trellis Q_OPT, Google jpegli444 (native AQ enabled)"
                                    if args.jpeg_native_suite else ("image baseline vs libjpeg-turbo progressive 444/420"
                                    if args.jpeg_suite else "image::codecs::jpeg::JpegEncoder")),
                   "cargo_features": features,
                   "native_linkage": "bundled shared TurboJPEG + static MozJPEG + prefixed static jpegli" if native_build else "bundled static TurboJPEG",
                   "target_search": ("all four methods log-secant" if args.jpeg_chroma_suite else
                                     ("T0/M0 binary; all other native methods bracketed log-secant" if args.jpeg_native_suite else "binary")),
                   "limits": {"max_pixels": 50000000, "working_memory_mib": 1024, "max_encode_attempts": 32} if args.jpeg_suite else "core defaults",
                   "quality_range": [40, 95], "resize": "all suite methods; independently selected dimensions" if args.jpeg_suite else ("explicit B1/E1-resize methods only" if args.samples else False),
                   "psnr_reference": "independent decoded original JPEG, EXIF-normalized; resized outputs restored with Lanczos3" if args.samples else "synthetic RGB pixels",
                   "prefilter": {"threshold": 0.04, "coherence_boost": 4.0},
                   "rate_coupling": "tau=min(max(base_tau,.25),base_tau*sqrt(max(1,unfiltered_q80_bytes/target_bytes))); q80 probe counts toward encode budget" if args.jpeg_suite and not args.jpeg_chroma_suite else False,
                   "cargo_lock_sha256": hashlib.sha256((ROOT / "Cargo.lock").read_bytes()).hexdigest(),
                   "cargo_toml_sha256": hashlib.sha256((ROOT / "Cargo.toml").read_bytes()).hexdigest(),
                   "build_rs_sha256": hashlib.sha256((ROOT / "build.rs").read_bytes()).hexdigest(),
                   "rust_source_sha256": {str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest()
                                          for folder in [ROOT / "src", ROOT / "examples"]
                                          for p in sorted(folder.rglob("*.rs"))},
                   "input_sha256": manifest}
    (output / "environment.json").write_text(json.dumps(environment, indent=2) + "\n")
    print("sample | method | bytes/target | median ms | RGB PSNR | status")
    for row in summary:
        print(f"{row['sample']} | {row['method']} | {row['output_bytes']}/{row['target_bytes']} | "
              f"{row['median_ms']:.2f} | {row['psnr_rgb']} | {row['status']}")
    print(f"Artifacts: {output}")


if __name__ == "__main__":
    main()
