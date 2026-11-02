#!/usr/bin/env python3
"""Controlled JPEG fixtures and Linux per-process CLI measurements."""
import argparse
import csv
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import statistics
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
METHODS = [
    ('turbo', ['--jpeg-backend', 'turbo444']),
    ('moz-dqt', ['--jpeg-backend', 'mozjpeg', '--adaptive-dqt']),
    ('jpegli', ['--jpeg-backend', 'jpegli']),
]


def worker(job):
    import resource
    env = os.environ.copy()
    env['LD_LIBRARY_PATH'] = job['library_dir'] + os.pathsep + env.get('LD_LIBRARY_PATH', '')
    start = time.perf_counter()
    run = subprocess.run(job['argv'], env=env, capture_output=True, text=True)
    elapsed = time.perf_counter()-start
    # This fresh helper has exactly one child; Linux ru_maxrss cannot accumulate across cases.
    usage = resource.getrusage(resource.RUSAGE_CHILDREN)
    return {'returncode': run.returncode, 'wall_seconds': elapsed,
            'user_seconds': usage.ru_utime, 'system_seconds': usage.ru_stime,
            'peak_rss_kib': usage.ru_maxrss, 'stderr': run.stderr, 'stdout': run.stdout}


def launch_worker(argv, library_dir):
    job = {'argv': argv, 'library_dir': str(library_dir.resolve())}
    run = subprocess.run([sys.executable, str(Path(__file__).resolve()), 'worker', json.dumps(job)],
                         capture_output=True, text=True, check=True)
    return json.loads(run.stdout)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def metadata(args):
    library = args.library_dir/'libturbojpeg.so.0'
    if not args.binary.is_file() or not library.is_file():
        raise ValueError('provide the release CLI and its matching bundled native library directory')
    return {'platform': platform.platform(), 'python': platform.python_version(),
            'binary': str(args.binary.resolve()), 'binary_sha256': digest(args.binary),
            'native_library': str(library.resolve()), 'native_library_sha256': digest(library),
            'script_sha256': digest(Path(__file__)), 'methods': METHODS,
            'resize': 'explicit permission; Lanczos3; minimum long edge256',
            'search': 'explicit log-secant', 'metric_process_included_in_timing': False}


def encode(args, source, output, target, flags):
    argv = [str(args.binary.resolve()), str(source.resolve()), '-o', str(output.resolve()),
            '--format', 'jpeg', '--target', f'{target}B', '--allow-resize',
            '--min-long-edge', '256', '--search', 'log-secant', *flags]
    measured = launch_worker(argv, args.library_dir)
    if measured['returncode']:
        raise RuntimeError(measured['stderr'])
    if output.stat().st_size > target:
        raise RuntimeError('CLI reported success above the byte target')
    report = dict(re.findall(r'(\w+)=(\S+)', measured['stderr']))
    if report.get('search') != 'Some(LogSecant)':
        raise RuntimeError('unexpected CLI search method')
    measured.update(output_bytes=output.stat().st_size, output_sha256=digest(output),
                    encode_attempts=int(report['encode_attempts']), dimensions=report['dimensions'],
                    resized=report['resized'])
    return measured


def make_fixtures(font):
    import numpy as np
    from PIL import Image, ImageDraw, ImageFont
    document = Image.new('RGB', (1024, 768), 'white')
    draw = ImageDraw.Draw(document)
    regions = {'text-small': (20, 20, 1004, 200),
               'text-medium': (20, 220, 1004, 440), 'text-large': (20, 470, 1004, 748)}
    text = ['DailyTools JPEG: detail, edges and readable text.',
            'Invoice AB-2026-1103 0123456789  Rp 123.456,78',
            'The quick brown fox jumps over the lazy dog.']
    for size, region in zip([12, 18, 28], regions.values()):
        x, y, _, bottom = region
        face = ImageFont.truetype(str(font), size, layout_engine=ImageFont.Layout.BASIC)
        for line in range((bottom-y)//(size+10)):
            fill = ['black', '#a00000', '#004080'][line % 3]
            draw.text((x, y+line*(size+10)), text[line % len(text)], font=face, fill=fill)
    x = np.arange(1024, dtype=np.float64)/1023
    gradient = np.empty((768, 1024, 3), dtype=np.uint8)
    gradient[:256] = np.round(x*255).astype(np.uint8)[None, :, None]
    gradient[256:512] = np.round(8+x*40).astype(np.uint8)[None, :, None]
    gradient[512:] = np.stack([np.round(x*255), np.round(x*180), np.round((1-x)*255)], axis=1)
    return {'document': {'image': document, 'rois': regions, 'targets': [64000, 128000]},
            'gradient': {'image': Image.fromarray(gradient), 'rois': {
                'gray-ramp': (0, 0, 1024, 256), 'dark-ramp': (0, 256, 1024, 512),
                'color-ramp': (0, 512, 1024, 768)}, 'targets': [8000, 16000]}}


def fixture_run(args):
    import numpy as np
    from jpeg_quality import load_rgb, measure, scipy, pillow_version
    info = metadata(args)
    info.update(numpy=np.__version__, scipy=scipy.__version__, Pillow=pillow_version,
                scorer_sha256=digest(Path(__file__).with_name('jpeg_quality.py')),
                ssim={'luma': '0.299R+0.587G+0.114B on nonlinear8bitRGB',
                      'window': 11, 'sigma': 1.5, 'K1': 0.01, 'K2': 0.03,
                      'data_range': 255, 'covariance': 'population', 'border_crop': 5,
                      'scale': 'full original resolution; no SSIM pre-downsample'},
                decoder='Pillow RGB + ImageOps.exif_transpose; resized outputs restored with Pillow Lanczos3')
    args.output.mkdir(parents=True, exist_ok=False)
    fixtures = make_fixtures(args.font)
    info.update(font=str(args.font), font_sha256=digest(args.font), fixtures={})
    methods = METHODS + [
        ('moz-dqt-rate', [*METHODS[1][1], '--adaptive-prefilter']),
        ('jpegli-rate', [*METHODS[2][1], '--adaptive-prefilter'])]
    if args.mode == 'chroma-fixtures':
        methods = [METHODS[0], ('turbo420', ['--jpeg-backend', 'turbo420']),
                   METHODS[2], ('jpegli420', ['--jpeg-backend', 'jpegli420'])]
    info['methods'] = methods
    rows, runs = [], []
    for name, fixture in fixtures.items():
        source = args.output/f'{name}.png'; fixture['image'].save(source)
        info['fixtures'][name] = {'file': source.name, 'sha256': digest(source),
                                  'rois': fixture['rois'], 'targets': fixture['targets']}
        reference = load_rgb(source)
        for target in fixture['targets']:
            for method, flags in methods:
                output = args.output/f'{name}-{target}-{method}.jpg'
                run = encode(args, source, output, target, flags)
                runs.append({'fixture': name, 'target': target, 'method': method, **run})
                decoded = load_rgb(output, fixture['image'].size)
                for roi_name, roi in {'whole': None, **fixture['rois']}.items():
                    result = measure(reference, decoded, roi=roi)
                    item = {'fixture': name, 'target_bytes': target, 'method': method,
                            'roi': roi_name, **result, 'output_bytes': run['output_bytes'],
                            'dimensions': run['dimensions'], 'resized': run['resized']}
                    if roi_name.startswith('text-'):
                        x0, y0, x1, y1 = roi
                        a, b = reference[y0:y1, x0:x1], decoded[y0:y1, x0:x1]
                        foreground = a @ np.array([0.299, 0.587, 0.114]) < 128
                        predicted = b @ np.array([0.299, 0.587, 0.114]) < 128
                        intersection = np.count_nonzero(foreground & predicted)
                        item.update(text_recall=intersection/max(1, np.count_nonzero(foreground)),
                                    text_precision=intersection/max(1, np.count_nonzero(predicted)),
                                    threshold_mismatch=float(np.mean(foreground != predicted)))
                    rows.append(item)
                print(f'fixture {name} target={target} {method} complete', flush=True)
    fields = list(dict.fromkeys(k for row in rows for k in row))
    with (args.output/'roi-quality.csv').open('w', newline='') as file:
        writer = csv.DictWriter(file, fieldnames=fields); writer.writeheader(); writer.writerows(rows)
    (args.output/'runs.json').write_text(json.dumps(runs, indent=2)+'\n')
    info['text_metric'] = 'binary foreground precision/recall at Y<128; not OCR or proof of readability'
    info['gradient_metric'] = 'SSIM/PSNR/MAE and saved ramps for inspection; no perceptual banding guarantee'
    (args.output/'environment.json').write_text(json.dumps(info, indent=2)+'\n')


def performance_run(args):
    cpu = min(os.sched_getaffinity(0))
    os.sched_setaffinity(0, {cpu})
    info = metadata(args)
    if args.repetitions < 3: raise ValueError('use at least three measured repetitions')
    cases = [('photo', args.samples/'pexels-markusspiske-105003.jpg', 200000),
             ('noise', args.samples/'sample-1mb.jpg', 200000),
             ('text', args.fixtures/'document.png', 64000)]
    info.update(repetitions=args.repetitions, warmups_per_case_method=1,
                cpu_affinity=sorted(os.sched_getaffinity(0)),
                rss_scope='fresh CLI child only; Linux RUSAGE_CHILDREN.ru_maxrss KiB; one child per helper',
                timing_scope='CLI process launch/load/read/compress/validate/write/fsync; no quality metrics',
                inputs={name: {'path': str(p.resolve()), 'sha256': digest(p), 'target_bytes': target}
                        for name, p, target in cases})
    args.output.mkdir(parents=True, exist_ok=False)
    rows = []
    for name, source, target in cases:
        for method, flags in METHODS:
            encode(args, source, args.output/f'{name}-{method}-warmup.jpg', target, flags)
        for iteration in range(args.repetitions):
            ordered = METHODS[iteration % len(METHODS):]+METHODS[:iteration % len(METHODS)]
            for method, flags in ordered:
                result = encode(args, source, args.output/f'{name}-{method}-{iteration}.jpg', target, flags)
                rows.append({'case': name, 'method': method, 'iteration': iteration, 'target_bytes': target, **result})
                print(f'performance {name} {method} run={iteration+1} {result["wall_seconds"]:.3f}s {result["peak_rss_kib"]}KiB', flush=True)
    with (args.output/'measurements.csv').open('w', newline='') as file:
        writer = csv.DictWriter(file, fieldnames=list(rows[0])); writer.writeheader(); writer.writerows(rows)
    summary = []
    for name, _, _ in cases:
        for method, _ in METHODS:
            group = [r for r in rows if r['case']==name and r['method']==method]
            if len({r['output_sha256'] for r in group}) != 1:
                raise RuntimeError('nondeterministic output between repetitions')
            summary.append({'case': name, 'method': method,
                            'median_wall_seconds': statistics.median(r['wall_seconds'] for r in group),
                            'min_wall_seconds': min(r['wall_seconds'] for r in group),
                            'max_wall_seconds': max(r['wall_seconds'] for r in group),
                            'median_peak_rss_kib': statistics.median(r['peak_rss_kib'] for r in group),
                            'max_peak_rss_kib': max(r['peak_rss_kib'] for r in group),
                            'output_bytes': group[0]['output_bytes'], 'dimensions': group[0]['dimensions'],
                            'encode_attempts': group[0]['encode_attempts']})
    (args.output/'summary.json').write_text(json.dumps(summary, indent=2)+'\n')
    for _, source, _ in cases:
        expected = next(v['sha256'] for v in info['inputs'].values() if v['path']==str(source.resolve()))
        if digest(source) != expected: raise RuntimeError('source changed during measurement')
    (args.output/'environment.json').write_text(json.dumps(info, indent=2)+'\n')


def main():
    if sys.platform != 'linux': raise SystemExit('this native measurement harness currently requires Linux')
    if len(sys.argv)>1 and sys.argv[1]=='worker':
        print(json.dumps(worker(json.loads(sys.argv[2])))); return
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=['fixtures', 'chroma-fixtures', 'performance'])
    parser.add_argument('--binary', type=Path, default=ROOT/'target/release/dt-img')
    parser.add_argument('--library-dir', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--font', type=Path, default=Path('/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf'))
    parser.add_argument('--samples', type=Path, default=ROOT/'samples')
    parser.add_argument('--fixtures', type=Path, default=ROOT/'samples/results/jpeg-fixtures-2026-11-03')
    parser.add_argument('--repetitions', type=int, default=3)
    args = parser.parse_args()
    if args.mode in ('fixtures', 'chroma-fixtures'): fixture_run(args)
    else: performance_run(args)


if __name__ == '__main__':
    main()
