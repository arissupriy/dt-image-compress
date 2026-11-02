# Execution ledger — plan: /home/arissupriy/Projects/DailyTools/PLANS.md

Scope: image compression only. Work locally while the user prepares the remote repositories.

- Ruling: use the independent `dt-image-compress/` source directory; there is no Git repository yet, so Git worktree/commit scripts do not apply. Do not create a parent Cargo workspace or a remote repository.
- Ruling: first deliver a JPEG/PNG library, CLI, reproducible baseline, and an opt-in tensor experiment. Additional image formats follow their own gates; Flutter/video are outside this implementation slice.
- Ruling: Rust MSRV 1.88, edition 2024; development toolchain 1.93.0. Pin reproducible dependency resolution in Cargo.lock. `image` 0.25.10 and `oxipng` 10.2.1 were checked against registry metadata and official documentation.
- Ruling: keep MIT OR Apache-2.0 as the proposed package licensing from PLANS.md; no publication is performed before the project owner finalizes distribution.
- Task T02–T05: RED tests written for input limits, target bytes, EXIF, transparency, and sample-exact PNG. Initial library has no implementation.

## Implemented slice

- T01 partial: API/limits, dependency decisions, research caveats, and benchmark protocol recorded. Full prior-art review, real evaluation corpus, final distribution audit, and registry/repository availability remain open.
- T02–T05 baseline: structured errors, validated JPEG/PNG input, estimated memory/decoder limits, bounded quality search, consent-based resize, EXIF 1–8, explicit alpha flatten, and sample-exact PNG (including 16-bit/hidden RGB).
- T06 baseline: optional `dt-img`, KB/KiB parser, bounded input, no-clobber temporary output publication, binary stdout, and cooperative SIGINT handling for blocked pipe I/O.
- T07 partial: correctly windowed tensor, opt-in tiled prefilter, numerical/edge/noise/seam tests, and reproducible synthetic/local-JPEG harnesses. Diagnostic full tensor maps remain capped at 1 MP. No rate-coupled threshold, DQT modification, log-secant, or custom SIMD yet.
- T14 partial: CI prepared for Rust 1.88/stable on Linux; remote CI, registry publication, and cross-platform distribution have not run.

## RED → GREEN evidence and review rulings

- Initial tests failed because public API was absent; baseline integration tests then passed.
- CLI tests failed against the empty binary, then passed after implementation.
- Tensor tests failed at unimplemented functions, then passed with Gaussian-windowed components.
- Independent review found blocked pipe cancellation, f32 threshold-square underflow, and silently accepted truncated JPEG entropy.
- Pipe cancellation regressions failed, then passed after cancellable I/O workers with 25 ms polling. Core cancellation remains cooperative between codec calls.
- Tiny normal/subnormal threshold regression turned uniform white black before the fix; f64 denominator calculation passed the same regression.
- Truncated JPEG regression failed with image/zune and with jpeg-decoder 0.3.2 even after an EOI marker was appended. Ruling: use turbojpeg 1.5.1 for pixel decode and reject its warnings; native build dependency is justified by integrity at the input boundary. The trial jpeg-decoder dependency was removed. Regression passed for cuts of 10/100 bytes and 25%/50%, with and without an added EOI marker.
- First complete native all-feature/all-target suite after fixes: 33 passed. Subsequent default-build check hit a disk-full linker failure, not a test assertion. Only this package's generated target/debug directory was removed; source, lockfile, and benchmark results were preserved. Remaining checks use debug symbols/incremental disabled for the constrained local disk.

Final command results and benchmark conclusions follow below.

## Final verification — 2 November 2026

- CMYK/YCCK valid input regression first failed as InvalidInput; native header guard now returns UnsupportedOperation, and README explicitly limits supported JPEG color spaces.
- Baseline and all-feature tests, rustfmt, and default/all-feature Clippy were run from the package with `--offline --locked`; local build environment set `CARGO_HOME=/tmp/dailytools-cargo-home`, `CARGO_INCREMENTAL=0`, and dev/test debug symbols to zero to fit available disk. These environment choices do not alter compression settings.
- Final `cargo test --offline --locked --all-features --all-targets -j 2`: **35 tests passed**. Default-feature test suite also passed, including **1 README/API doctest**. `cargo fmt --check` and default/all-feature `cargo clippy --all-targets -- -D warnings`: PASS.
- `cargo build --offline --locked --release --all-features -j 2`: PASS. Local executable is `target/release/dt-img`; 2,447,488 bytes, no remote publication.
- `cargo package --offline --locked --no-verify --allow-dirty`: package assembly PASS; this is not registry publication or a claim that packaging verification/remote CI ran.
- `python3 tools/benchmark.py --self-test`: PASS. Release benchmark: 45 rows, equal byte budgets, sample-exact transparent PNG, recorded Linux VmHWM. Raw results and source hashes preserved under `docs/benchmarks/2026-11-02-smoke/`.
- Decision: tensor prefilter remains opt-in. Synthetic size reduction 0–1.631% did not meet the proposed 5% gate; PSNR slightly regressed on several fixtures and elapsed time increased. See the benchmark report; no novelty or mobile-performance claim.
- Remaining: real licensed corpus, broader prior-art/ablation study, fuzzing, Android/ARM64, actual Rust 1.88 CI execution, additional formats, final dependency distribution audit, and any remote Git/registry work.

## JPEG-only follow-up — user samples, 2 November 2026

- User requested JPEG focus and a check of combined algorithms against PLANS.md. Three JPEGs in samples/ were inspected locally: 3.69 MP and two 8.29 MP inputs. They appear to be variants of the same noisy mountain illustration, not independent content classes. No original sample is uploaded or copied into reports.
- RED: >1MP JPEG prefilter integration failed with ResourceLimitExceeded("experimental scratch buffers"). GREEN: prefilter now processes 256x256 tiles with a 2px halo, preserving original resolution and using an estimated two RGB frames plus 8 MiB tile scratch. Full diagnostic tensor maps still have the 1MP cap. Seam/edge equality against the full reference and cancellation between tiles pass.
- Benchmark checks first failed when different targets were merged into one summary group and when PSNR silently accepted mismatched dimensions. Summary now groups by target/status; PSNR rejects mismatched dimensions. Python self-check also covers unreachable cases and oversized successes.
- Release local-JPEG benchmark: 36 sample/target/method combinations, one warm-up plus three repetitions each, 108 measured rows. 24 combinations succeeded, all within target; 12 returned TargetUnreachable. All 18 combinations allowing resize succeeded, using at most 19 encodes. Whole benchmark process peak VmHWM 200,012 KiB, including metric/reference buffers outside core timing. Timing was variable on the non-isolated host; no latency/speed-up claim.
- Raw CSV/summary/environment/RSS copied to docs/benchmarks/2026-11-02-jpeg-samples/. Generated JPEGs remain under ignored target/jpeg-samples-2026-11-02/. Cargo --list confirms no original samples/JPEGs in package; explicit package exclude protects samples even before Git initialization.
- Independent focused review found no important issues in tiling, memory estimates, cancellation, or metric bookkeeping. README/protocol were updated; comparisons between resized methods explicitly account for independently chosen dimensions.
- Verification: cargo test --offline --locked --all-features --all-targets -j 2: **39 passed**. Default cargo test (including README doctest), fmt, default/all-feature Clippy -D warnings, benchmark Python self-check, and release CLI/example builds passed.
- Decision: E1 remains opt-in. At target 1 MB the 1 MB source gained about 1.01 dB PSNR with E1; at 500 KB it lost 0.141 dB. Higher encoder quality or larger output dimensions alone do not prove better fidelity. No rate-coupled threshold, custom DQT, alternative encoder, OCR/SSIM, or Android validation was added. See docs/benchmarks/jpeg-samples-2026-11-02.md for the full plan gap matrix and results.
