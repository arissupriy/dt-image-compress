"""Score new chroma pairs; reuse old scores only for identical source/JPEG hashes."""
import argparse
import csv
import hashlib
import json
from pathlib import Path
import platform
import statistics
import sys
import time

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT/'tools'))
import jpeg_quality as quality

sha = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('run', type=Path)
parser.add_argument('output', type=Path)
args = parser.parse_args()
old_dir = ROOT/'docs/benchmarks/2026-11-03-jpeg-ssim'
old = json.loads((old_dir/'environment.json').read_text())
assert sha(ROOT/'tools/jpeg_quality.py') == old['scorer_sha256'], 'scoring protocol changed'
versions = {'numpy': quality.np.__version__, 'scipy': quality.scipy.__version__,
            'Pillow': quality.pillow_version}
assert all(old[k] == value for k, value in versions.items()), 'dependency versions changed'
cache = {}
for row in csv.DictReader((old_dir/'quality.csv').open()):
    key = old['inputs'][row['sample']]['sha256'], row['output_sha256']
    cache[key] = {k: float(row[k]) for k in ['ssim_y', 'psnr_rgb', 'mae_rgb']}
env = json.loads((args.run/'environment.json').read_text())
assert env['input_sha256'] == old['inputs'], 'source corpus changed'
source_rows = list(csv.DictReader((args.run/'results.csv').open()))
args.output.mkdir(parents=True, exist_ok=False)
rows, reference, current_sample, measured, reused = [], None, None, 0, 0
started = time.monotonic()
for row in source_rows:
    assert row['status'] == 'ok'
    sample, method = row['sample'], row['method']
    info = env['input_sha256'][sample]
    filename = info['filename']; assert Path(filename).name == filename
    source = ROOT/'samples'/filename
    output = args.run/f'{Path(filename).stem}-{row["target_bytes"]}-{method}.jpg'
    assert output.stat().st_size == int(row['output_bytes']) <= int(row['target_bytes'])
    assert sha(source) == info['sha256']
    output_sha = sha(output)
    key = info['sha256'], output_sha
    cached = key in cache
    if not cached:
        if sample != current_sample:
            reference = quality.load_rgb(source); current_sample = sample
        decoded = quality.load_rgb(output, (reference.shape[1], reference.shape[0]))
        cache[key] = quality.measure(reference, decoded)
        measured += 1
    else:
        reused += 1
    rows.append({'sample': sample, 'method': method, 'status': 'ok',
                 'output_bytes': row['output_bytes'], 'output_sha256': output_sha,
                 **cache[key], 'cached_identical_jpeg': cached})
    print(sample, method, 'cached' if cached else 'measured', flush=True)
assert len(rows) == len(env['input_sha256'])*4
for info in env['input_sha256'].values():
    assert sha(ROOT/'samples'/info['filename']) == info['sha256']
with (args.output/'quality.csv').open('w', newline='') as file:
    writer = csv.DictWriter(file, fieldnames=list(rows[0])); writer.writeheader(); writer.writerows(rows)
summary = {'methods': {}, 'pairs': {}, 'tie_tolerance': 1e-12}
for method in sorted({r['method'] for r in rows}):
    group = [r for r in rows if r['method'] == method]
    summary['methods'][method] = {k: statistics.mean(r[k] for r in group)
                                  for k in ['ssim_y', 'psnr_rgb', 'mae_rgb']}
for a, b in [('J2-jpegli420-log', 'J0-jpegli-log'), ('J2-jpegli420-log', 'T1-turbo-log'),
             ('J2-jpegli420-log', 'T2-turbo420-log'), ('T2-turbo420-log', 'T1-turbo-log')]:
    group = {}
    for metric in ['ssim_y', 'psnr_rgb']:
        left = {r['sample']: r[metric] for r in rows if r['method'] == a}
        right = {r['sample']: r[metric] for r in rows if r['method'] == b}
        assert set(left) == set(right) == set(env['input_sha256'])
        delta = [left[s]-right[s] for s in left]
        group[metric] = {'wins': sum(x > 1e-12 for x in delta),
                         'ties': sum(abs(x) <= 1e-12 for x in delta),
                         'losses': sum(x < -1e-12 for x in delta),
                         'mean_delta': statistics.mean(delta), 'min_delta': min(delta), 'max_delta': max(delta)}
    summary['pairs'][a+' vs '+b] = group
(args.output/'summary.json').write_text(json.dumps(summary, indent=2)+'\n')
metadata = {'platform': platform.platform(), 'python': platform.python_version(), **versions,
            'rows': len(rows), 'newly_measured': measured, 'cached_identical_pairs': reused,
            'elapsed_seconds': time.monotonic()-started,
            'scorer_sha256': old['scorer_sha256'], 'runner_sha256': sha(Path(__file__)),
            'cache_csv_sha256': sha(old_dir/'quality.csv'),
            'cache_environment_sha256': sha(old_dir/'environment.json'),
            'source_csv_sha256': sha(args.run/'results.csv'),
            'source_environment_sha256': sha(args.run/'environment.json'),
            'ssim': old['ssim'], 'decoder': old['decoder'], 'reencoded': False,
            'cache_rule': 'identical source and encoded JPEG SHA-256; identical scorer and dependency versions'}
(args.output/'environment.json').write_text(json.dumps(metadata, indent=2)+'\n')
