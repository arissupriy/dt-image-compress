# JPEG hybrid: 13 gambar, 7 metode, target 200 KB

**91/91 kombinasi berhasil**, seluruhnya tidak lebih dari200.000 bytes dan tidak lebih dari32 encode. Seluruh13 file sumber diverifikasi tetap sama melalui SHA-256. Encoder teroptimasi memberi perbaikan yang lebih konsisten daripada penambahan filter. Coupling sudah diimplementasikan dan diuji, tetapi belum menunjukkan keuntungan rata-rata yang cukup untuk menjadi default.

## Kesesuaian dengan rencana

Bagian JPEG yang sudah dijalankan: Scharr → Gaussian-windowed structure tensor → bobot energi/koherensi → convex blend; kontrol Gaussian sederhana/B2; filter tetap/E1; filter mengikuti tekanan target/E2; encoder pembanding/E3; bounded quality search; resize Lanczos3 yang diizinkan; output decode validation. Tensor diproses per tile256×256 dengan halo2, bukan menurunkan resolusi diam-diam sebelum analisis.

Encoder default adalah **libjpeg-turbo optimized Huffman + progressive, 4:4:4**. Backend4:2:0 tersedia secara eksplisit. `image::JpegEncoder` dipertahankan sebagai baseline, bukan dijadikan satu-satunya pembuktian kualitas. Ini belum mencakup MozJPEG/jpegli, custom adaptive DQT, optimasi SIMD khusus, atau log-secant. Tidak ada klaim bahwa gabungan ini baru atau optimal secara global.

H2: satu encode q80 tanpa filter mengukur `rho=max(1,B_q80/B_target)`, kemudian `tau=min(max(tau_base,.25),tau_base*sqrt(rho))`. Probe dihitung dalam budget32 encode. Filter dibekukan selama pencarian quality/skala. Referensi q80 yang sudah feasible dipertahankan sebagai fallback bila budget sisanya gagal menghasilkan kandidat terfilter. Threshold terukur tersedia di CSV.

## Corpus dan perbandingan

Corpus berisi10 JPEG Pexels yang disediakan pengguna dan3 variasi ilustrasi/noise awal. Beberapa foto mencapai24–40MP. Tidak ada profil ICC terdeteksi; orientasi EXIF dinormalisasi. File asli tidak disalin ke Git, diubah, atau diunggah.

Semua metode memakai target200.000 bytes, quality40–95, maksimal32 encode, resize opsional yang pada putaran ini **diizinkan untuk semuanya**, serta minimum long edge1.024. Dimensi dipilih independen oleh setiap pipeline dan dicatat. Mode benchmark desktop memakai limit eksplisit50MP/1GiB, sedangkan default library tetap20MP/256MiB. Referensi PSNR adalah JPEG asli yang didecode independen, bukan RAW/lossless. Output resize dipulihkan ke dimensi sumber dengan Lanczos3 sebelum PSNR.

| Metode | Backend | Filter |
| --- | --- | --- |
| B0-image | image baseline | Tidak ada |
| E3-turbo444 | TurboJPEG progressive444 | Tidak ada |
| E3-turbo420 | TurboJPEG progressive420 | Tidak ada |
| B2-gaussian | TurboJPEG progressive444 | Gaussian sigma .8 |
| E1-fixed | TurboJPEG progressive444 | Tensor, tau .04, coherence boost4 |
| E2-rate | TurboJPEG progressive444 | Tensor, tau mengikuti tekanan target |
| E2-rate420 | TurboJPEG progressive420 | Tensor, tau mengikuti tekanan target |

## Hasil agregat

PSNR lebih tinggi berarti error RGB lebih rendah terhadap referensi tersebut. Jumlah menang/kalah berikut hanya berlaku pada13 input dan target200KB ini.

| Metode | Menang/kalah vs B0 | Rata-rata delta PSNR dB | Median delta dB |
| --- | --- | ---: | ---: |
| E3-turbo444 | 12/1 | +0,2761 | +0,1324 |
| E3-turbo420 | 10/3 | +0,2440 | +0,0936 |
| B2-gaussian | 10/3 | +0,0224 | +0,0633 |
| E1-fixed | 12/1 | +0,2799 | +0,1102 |
| E2-rate | 12/1 | +0,2712 | +0,1113 |
| E2-rate420 | 9/4 | +0,2499 | +0,0759 |

Kontribusi coupling perlu dipisahkan dari kontribusi encoder:

- E2-rate vs Turbo444 tanpa filter:9 menang/4 kalah, rata-rata **−0,0049dB**, median+0,0068dB.
- E2-rate vs E1-fixed:8 menang/5 kalah, rata-rata **−0,0087dB**, median+0,0022dB.
- E2-rate420 vs Turbo420 tanpa filter:8 menang/5 kalah, rata-rata+0,0059dB.

Contoh yang perlu dilihat bersama:

- `pexels-zhanqun-cai-1507025-7640750.jpg`: B0 35,7048dB; Turbo444 37,7087dB; Turbo420 38,9175dB. Keuntungan utama berasal dari backend/pipeline, bukan coupling.
- `pexels-izafi-33061425.jpg`: B0 34,5721dB; Turbo444 34,4491dB. Baseline lama tetap menang pada kasus ini; hasil tersebut dipertahankan.
- `pexels-tizzy-20472033.jpg`: Turbo420 turun0,9402dB terhadap B0, sedangkan E2-rate444 naik0,1113dB. Subsampling420 tidak aman dijadikan pemenang universal.
- `sample-1mb.jpg`: pada200KB, B0 197.973 bytes /30,1011dB; E2-rate420 195.239 bytes /30,2891dB, tetap2560×1440 pada jalur adaptif.

Percobaan CLI tambahan pada `sample-1mb.jpg` memakai target100KB, Turbo420 dan coupling: **98.345 bytes, 2560×1440, tanpa resize, quality40,8 encode**. PSNR ImageMagick tercatat29,8982dB. Ini perubahan lossy, bukan kualitas identik. Percobaan entropy-only terpisah terhadap output baseline500KB memberi487.640→428.539 bytes dengan decoded-pixel AE=0; jangan menyamakan hasil entropy-only itu dengan semua perubahan backend/filter.

## Artefak dan keputusan

- Hasil lengkap lokal: `/home/arissupriy/Projects/DailyTools/dt-image-compress/samples/results/jpeg-suite-2026-11-02/`.
- Satu hasil per gambar dengan PSNR tertinggi: `/home/arissupriy/Projects/DailyTools/dt-image-compress/samples/results/best-psnr-200KB/`. Pemilihan ini dilakukan setelah pengukuran, bukan fitur auto-selection dalam engine dan bukan jaminan kualitas perseptual tertinggi. `selection.csv` mencatat metode yang dipilih.
- Data versioned: [CSV](2026-11-02-jpeg-suite/results.csv), [ringkasan](2026-11-02-jpeg-suite/summary.json), [manifest](2026-11-02-jpeg-suite/environment.json), [pilihan per gambar](2026-11-02-jpeg-suite/selection.csv), [RSS](2026-11-02-jpeg-suite/peak-rss-kib.txt). Label resize di manifest dikoreksi agar sesuai kenyataan bahwa seluruh metode suite mengizinkan resize; metrik tidak diubah.

Peak RSS proses benchmark825.840KiB (sekitar806,5MiB), termasuk referensi dan metrik; bukan RSS core per gambar. Satu repetisi tanpa warmup dan aktivitas build lain pada mesin yang sama membuat waktu tercatat tidak layak dipakai untuk klaim latency/speed-up. Hash seluruh source Rust yang tercatat cocok dengan source yang diuji.

**Keputusan:** gunakan encoder teroptimasi444 sebagai default, pertahankan filter tetap/adaptif dan420 sebagai pilihan eksplisit. H2 sudah dieksekusi sesuai hipotesis dan ablation, tetapi hasilnya belum mendukung default filter. Lanjutkan evaluasi perseptual/ROI, corpus terpisah untuk tuning/evaluasi, serta pembanding MozJPEG/jpegli sebelum klaim lebih luas. SSIM/OCR, studi pembacaan manusia terkontrol, Android/ARM64, dan validasi MSRV belum dikerjakan pada putaran ini.

```sh
python3 tools/benchmark.py --samples samples --jpeg-suite --iterations 1 --output samples/results/jpeg-suite-run
```

Direktori output wajib baru. Verifikasi lokal akhir:47 tes all-feature/all-target, tes default termasuk doctest, format, Clippy default/all-feature, serta release CLI lulus.
