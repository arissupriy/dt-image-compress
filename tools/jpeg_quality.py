#!/usr/bin/env python3
"""Offline quality evaluation of existing JPEG outputs; never re-encodes inputs."""
import argparse
import csv
import hashlib
import json
import math
from pathlib import Path
import platform
import time

import numpy as np
import scipy
from scipy.ndimage import gaussian_filter
from PIL import Image, ImageOps, __version__ as pillow_version


def measure(reference, output, roi=None, tile=256):
    if (reference.shape != output.shape or reference.dtype != np.uint8 or
            output.dtype != np.uint8 or reference.ndim != 3 or reference.shape[2] != 3 or
            not isinstance(tile, int) or tile <= 0):
        raise ValueError('expected matching uint8 RGB images and a positive tile size')
    h, w = reference.shape[:2]
    if roi is not None:
        x0, y0, x1, y1 = roi
        if not (0 <= x0 < x1 <= w and 0 <= y0 < y1 <= h):
            raise ValueError('ROI must lie inside the image')
        reference, output = reference[y0:y1, x0:x1], output[y0:y1, x0:x1]
        h, w = reference.shape[:2]
    if min(h, w) < 11 or h*w > 50_000_000:
        raise ValueError('SSIM needs dimensions >=11 and at most50MP')
    squared, absolute = 0, 0
    for y in range(0, h, tile):
        difference = reference[y:y+tile].astype(np.int16) - output[y:y+tile].astype(np.int16)
        squared += int(np.square(difference.astype(np.int32)).sum(dtype=np.int64))
        absolute += int(np.abs(difference).sum(dtype=np.int64))
    mse = squared / (h*w*3)
    total, count = 0.0, 0
    weights = np.array([0.299, 0.587, 0.114], dtype=np.float64)
    def gaussian(values):
        return gaussian_filter(values, 1.5, radius=5, output=np.float64)[5:-5, 5:-5]
    for y in range(5, h-5, tile):
        end_y = min(y+tile, h-5)
        for x in range(5, w-5, tile):
            end_x = min(x+tile, w-5)
            a = reference[y-5:end_y+5, x-5:end_x+5].astype(np.float64) @ weights
            b = output[y-5:end_y+5, x-5:end_x+5].astype(np.float64) @ weights
            mu, mv = gaussian(a), gaussian(b)
            vu = np.maximum(0.0, gaussian(a*a) - mu*mu)
            vv = np.maximum(0.0, gaussian(b*b) - mv*mv)
            cov = gaussian(a*b) - mu*mv
            score = ((2*mu*mv+6.5025)*(2*cov+58.5225) /
                     ((mu*mu+mv*mv+6.5025)*(vu+vv+58.5225)))
            total += float(score.sum(dtype=np.float64))
            count += score.size
    return {'ssim_y': total/count,
            'psnr_rgb': math.inf if mse == 0 else 10*math.log10(255**2/mse),
            'mae_rgb': absolute/(h*w*3)}


def load_rgb(path, size=None):
    with Image.open(path) as image:
        if image.width*image.height > 50_000_000:
            raise ValueError('image exceeds50MP')
        image = ImageOps.exif_transpose(image).convert('RGB')
        if size is not None and image.size != size:
            image = image.resize(size, Image.Resampling.LANCZOS)
        return np.asarray(image)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def evaluate(samples, run, output):
    env = json.loads((run/'environment.json').read_text())
    inputs = env['input_sha256']
    rows = list(csv.DictReader((run/'results.csv').open()))
    output.mkdir(parents=True, exist_ok=False)
    fields = ['sample', 'method', 'status', 'output_bytes', 'output_sha256',
              'ssim_y', 'psnr_rgb', 'mae_rgb', 'cached_identical_jpeg']
    scored, cache, current_sample, reference = [], {}, None, None
    started = time.monotonic()
    with (output/'quality.csv').open('w', newline='') as file:
        writer = csv.DictWriter(file, fieldnames=fields); writer.writeheader()
        for row in rows:
            if row['status'] != 'ok':
                raise ValueError('quality run requires successful source rows')
            sample = row['sample']; info = inputs[sample]; filename = info['filename']
            if Path(filename).name != filename:
                raise ValueError('source filename must be a basename')
            if sample != current_sample:
                if sha(samples/filename) != info['sha256']:
                    raise ValueError(f'source hash changed: {filename}')
                reference = load_rgb(samples/filename); current_sample = sample
            encoded = run/f'{Path(filename).stem}-{row["target_bytes"]}-{row["method"]}.jpg'
            if encoded.stat().st_size != int(row['output_bytes']):
                raise ValueError(f'output size changed: {encoded}')
            digest = sha(encoded); key = (info['sha256'], digest)
            cached = key in cache
            if not cached:
                restored = load_rgb(encoded, (reference.shape[1], reference.shape[0]))
                cache[key] = measure(reference, restored)
                del restored
            item = {k: row[k] for k in ['sample', 'method', 'status', 'output_bytes']}
            item.update(output_sha256=digest, **cache[key], cached_identical_jpeg=cached)
            writer.writerow(item); file.flush(); scored.append(item)
            print(f'{len(scored)}/{len(rows)} {sample} {row["method"]} SSIM={item["ssim_y"]:.6f}', flush=True)
    for info in inputs.values():
        if sha(samples/info['filename']) != info['sha256']:
            raise ValueError('input changed during evaluation')
    metadata = {'platform': platform.platform(), 'python': platform.python_version(),
                'numpy': np.__version__, 'scipy': scipy.__version__, 'Pillow': pillow_version,
                'source_run': str(run.resolve()), 'source_csv_sha256': sha(run/'results.csv'),
                'source_environment_sha256': sha(run/'environment.json'),
                'scorer_sha256': sha(Path(__file__)), 'elapsed_seconds': time.monotonic()-started,
                'rows': len(scored), 'unique_measured_jpegs': len(cache), 'inputs': inputs,
                'ssim': {'luma': '0.299R+0.587G+0.114B on nonlinear8bitRGB',
                         'window': 11, 'sigma': 1.5, 'K1': 0.01, 'K2': 0.03,
                         'data_range': 255, 'covariance': 'population', 'border_crop': 5,
                         'scale': 'full original resolution; no SSIM pre-downsample'},
                'decoder': 'Pillow RGB + ImageOps.exif_transpose; resized outputs restored with Pillow Lanczos3',
                'psnr_protocol': 'recomputed with this decoder/restoration; distinct from prior Rust image scores',
                'reencoded': False}
    (output/'environment.json').write_text(json.dumps(metadata, indent=2)+'\n')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--samples', type=Path, required=True)
    parser.add_argument('--run', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    evaluate(args.samples, args.run, args.output)
