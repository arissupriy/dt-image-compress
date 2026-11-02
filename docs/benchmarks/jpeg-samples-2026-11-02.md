# JPEG pengguna: kecocokan dengan PLANS.md

Snapshot pengujian awal, sebelum encoder default diganti dengan TurboJPEG progressive. Gabungan awal sudah berjalan, tetapi eksperimen JPEG dalam T07 belum selesai. Pipeline ini belum membuktikan algoritma baru atau keunggulan umum.

## Algoritma yang digabungkan

Alur aktual: pemeriksaan input → decode dan normalisasi EXIF → prefilter tensor opsional → pencarian quality → resize Lanczos3 bila diizinkan dan diperlukan → encode JPEG → decode validasi dan pemeriksaan batas bytes.

| Bagian rencana | Implementasi saat ini |
| --- | --- |
| Rate control dengan batas bytes | Binary search quality 40–95, menyimpan kandidat feasible terbaik yang diukur, maksimal 32 encode seluruh skala. |
| Resize adaptif | Lanczos3 dari sumber yang dinormalisasi, tanpa upscale; hanya dengan izin resize. Long edge minimum default 1.024 piksel. |
| Struktur lokal | Scharr 3×3/32 → outer product → Gaussian window `[1,2,1]²/16` pada tensor → energi dan koherensi. |
| Prefilter pelestarian struktur | Convex blend dengan Gaussian blur sigma 0,8. Bobot menggunakan energi/koherensi; threshold 0,04 dan coherence boost 4 tetap. Eksperimental, mati secara default. |
| Scratch terbatas | Tile 256×256, halo 2 piksel. Output dibandingkan pixel-exact dengan referensi penuh pada fixture noise/ramp, batas tile, tepi, dan gambar satu baris/kolom. Cancel antar-tile. |
| Encoder/decoder | Encode `image::JpegEncoder`; decode piksel input libjpeg-turbo. Pemakaian TurboJPEG sebagai decoder bukan pemakaian jpegli/MozJPEG sebagai encoder. |
| H2: threshold mengikuti tekanan target | **Belum diimplementasikan.** Ukuran target mengendalikan quality dan skala, belum kekuatan filter. |
| Log-secant | Belum; rencana memang menetapkannya sebagai optimasi setelah binary baseline. |
| Adaptive DQT/chroma, backend alternatif | Belum diaudit/diimplementasikan. Bukan syarat baseline v0.1 dan tidak diklaim sudah ada. |

Tile memungkinkan pengujian filter pada JPEG di atas 1 megapiksel tanpa menurunkan resolusi secara tersembunyi. API diagnostik `experimental::analyze` tetap dibatasi 1 megapiksel karena mengembalikan seluruh peta tensor; prefilter kompresi mengikuti batas core dan mempunyai batas atas 20 megapiksel. Estimasi memori bukan hard cap RSS.

## Sampel dan metode

| Alias | File asli | Bytes | Dimensi |
| --- | --- | ---: | --- |
| sample-01 | sample-10mb.jpg | 9.962.054 | 3840×2160 |
| sample-02 | sample-1mb.jpg | 1.022.240 | 2560×1440 |
| sample-03 | sample-5mb.jpg | 5.127.247 | 3840×2160 |

Ketiganya tampak memakai ilustrasi pegunungan yang sama dengan resolusi/noise berbeda. Sampel 10 MB sangat berisik. Ketiganya JPEG baseline 8-bit, tiga komponen, tanpa EXIF atau ICC terdeteksi. Ini bukan tiga jenis konten independen, bukan corpus foto/dokumen, dan tidak menguji keterbacaan teks.

Setiap sampel diuji pada target **200.000, 500.000, dan 1.000.000 bytes**. Metode:

- **B0:** quality search, dimensi asli, tanpa filter.
- **B1-resize:** B0 dengan izin Lanczos3 bila target gagal pada dimensi asli.
- **E1-tensor:** quality search + tensor, dimensi asli.
- **E1-tensor-resize:** tensor + quality search + izin Lanczos3.

Satu warm-up dan tiga repetisi per kombinasi. Bandingkan B0/E1 pada dimensi asli untuk menilai filter saja. Dua metode resize memilih dimensi secara independen; perbedaannya merupakan hasil seluruh pipeline. Tidak ada padding untuk mendekati target.

Timer hanya meliputi core. RGB PSNR memakai JPEG asli yang didecode independen sebagai referensi, bukan sumber RAW/lossless. Output resize dipulihkan ke dimensi asli dengan Lanczos3 untuk mengukur kehilangan detail. PSNR terhadap noise asli dapat menghukum denoising yang terlihat lebih halus; angka tersebut tidak menggantikan penilaian visual atau metrik perseptual.

## Hasil

**108 baris pengukuran, 36 kombinasi unik:** 24 kombinasi sukses (72 repetisi), 12 kombinasi `TargetUnreachable` (36 repetisi). Semua output sukses memenuhi target. Seluruh 18 kombinasi yang mengizinkan resize berhasil; izin resize tidak dipakai ketika dimensi asli sudah memenuhi target. Tidak ada output sukses yang diam-diam melewati budget.

Perbandingan target **500.000 bytes**:

| Input | Metode | Output bytes | Dimensi aktual | Quality | PSNR RGB dB |
| --- | --- | ---: | --- | ---: | ---: |
| 10 MB | B1-resize | 492.916 | 1920×1080 | 40 | 14,8075 |
| 10 MB | E1-tensor-resize | 489.923 | 1920×1080 | 40 | 14,8058 |
| 1 MB | B1-resize | 487.640 | 2560×1440 | 70 | 31,2191 |
| 1 MB | E1-tensor-resize | 493.417 | 2560×1440 | 76 | 31,0781 |
| 5 MB | B1-resize | 491.687 | 2657×1495 | 59 | 26,7918 |
| 5 MB | E1-tensor-resize | 498.951 | 2936×1652 | 52 | 26,8304 |

Tanpa resize, sampel 10 MB dan 5 MB gagal mencapai 500 KB pada kedua metode. Ukuran terkecil yang sempat diamati: 10 MB → B0 3.408.092 / E1 3.323.485 bytes; 5 MB → B0 941.967 / E1 771.531 bytes. Ini batas pencarian yang diuji, bukan minimum global JPEG.

Kasus penting yang memperlihatkan hasil bercampur:

- **1 MB → target 1 MB, resolusi asli:** B0 965.461 bytes / 32,0026 dB; E1 955.275 bytes / 33,0124 dB. Tensor unggul sekitar 1,01 dB pada kasus ini.
- **1 MB → target 500 KB, resolusi asli:** E1 mempunyai angka quality lebih tinggi (76 vs 70), tetapi PSNR turun 0,141 dB dan file lebih besar. Quality encoder tidak mengukur kerusakan akibat prefilter.
- **1 MB → target 200 KB:** B1 197.973 bytes pada 2087×1174 / 30,1011 dB; E1 188.999 bytes pada 2304×1296 / 30,0623 dB. Dimensi lebih besar tidak otomatis membuat fidelity lebih baik.
- **5 MB → target 1 MB, resolusi asli:** B0 980.788 bytes / 27,3240 dB; E1 999.398 bytes / 27,4356 dB. PSNR sedikit naik, dengan file lebih besar.
- **10 MB yang sangat berisik:** target 200 KB–1 MB membutuhkan resize pada kedua metode. PSNR rendah terhadap JPEG asli menandakan perubahan besar; jangan menyebut hasilnya tanpa kehilangan kualitas.

Peak RSS seluruh proses benchmark: **200.012 KiB** (sekitar 195,3 MiB), termasuk gambar referensi dan perhitungan metrik. Ini bukan peak core per gambar. Median waktu tiap kombinasi tersedia di CSV/ringkasan; tiga repetisi pada mesin yang tidak diisolasi menunjukkan variasi besar, sehingga hasil ini belum menjadi dasar klaim latency atau speed-up, apalagi performa mobile.

Data lengkap: [results.csv](2026-11-02-jpeg-samples/results.csv), [summary.json](2026-11-02-jpeg-samples/summary.json), [environment.json](2026-11-02-jpeg-samples/environment.json), dan [RSS](2026-11-02-jpeg-samples/peak-rss-kib.txt). Manifest mencatat hash Rust/Cargo.lock dan input. Gambar hasil lokal berada di `/home/arissupriy/Projects/DailyTools/dt-image-compress/target/jpeg-samples-2026-11-02/`.

**Keputusan:** pertahankan tensor sebagai eksperimen opt-in. Gabungan Scharr, Gaussian-windowed tensor, blend, quality search, dan Lanczos sudah dapat diuji pada sampel utuh; H2/rate coupling masih belum ada. Berikutnya tetap fokus JPEG: bandingkan denoise sederhana (B2), uji threshold mengikuti target (E2) dengan filter dibekukan dalam setiap pencarian quality dan budget encode tetap berbatas, lalu audit backend alternatif (E3). Teks/detail ROI serta corpus yang beragam diperlukan sebelum memilih konfigurasi default.

## Reproduksi

```sh
python3 tools/benchmark.py --samples samples --iterations 3 --output target/jpeg-samples-run
```

Direktori output harus belum ada. File asli tidak diubah/disalin/diunggah. Gambar hasil tetap lokal di `target/`, sedangkan laporan hanya merujuk ukuran, metrik, dan manifest hash. Direktori `samples/` diabaikan Git dan dikecualikan dari paket Cargo.
