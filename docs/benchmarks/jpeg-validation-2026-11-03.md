# Validasi JPEG: ROI, waktu/RAM dan Android — 3 November 2026

Konfigurasi default tetap **Turbo444 + log-secant tanpa prefilter**. Untuk build desktop yang mendukung dependency native, **jpegli + log-secant tanpa prefilter** menjadi opsi kualitas berdasarkan SSIM corpus dan ROI teks kecil. MozJPEG/adaptive DQT tetap pilihan eksplisit; tensor rate-coupled tetap eksperimental. Keputusan ini memakai hasil yang terukur, tanpa menambah preset atau pemilih backend otomatis.

## Fixture teks dan gradien

Dua referensi PNG 1024×768 dibuat deterministik: dokumen Latin dengan teks hitam/merah/biru ukuran 12/18/28, serta tiga band gradien abu-abu, gelap dan warna. Font DejaVuSans, hash font/referensi, batas ROI, binary dan native library disimpan. Ada lima metode × dua target × dua fixture = **20 JPEG**, semuanya memenuhi target; skor seluruh gambar dan tiga ROI menghasilkan **80 baris metrik**. Resize diizinkan eksplisit, minimum long edge 256; SSIM/PSNR dihitung setelah pemulihan ukuran referensi dengan protokol [laporan SSIM](jpeg-ssim-2026-11-03.md).

Tabel dokumen menampilkan ketiga backend tanpa prefilter. Recall adalah proporsi piksel foreground referensi yang tetap di bawah ambang luma128 setelah restorasi; **bukan akurasi OCR atau bukti keterbacaan**.

| Target | Metode | Bytes | Dimensi tersimpan | SSIM seluruh | SSIM ROI teks kecil | Recall foreground kecil |
| --- | --- | ---: | --- | ---: | ---: | ---: |
| 64.000 | Turbo444 | 63.984 | 881×661 | 0,968232 | 0,955702 | 0,801754 |
| 64.000 | MozJPEG + DQT | 62.779 | 921×691 | 0,973172 | 0,960277 | 0,715974 |
| 64.000 | jpegli | 63.427 | 898×674 | 0,971963 | 0,961685 | 0,787266 |
| 128.000 | Turbo444 | 126.642 | 1024×768 | 0,991407 | 0,991804 | 0,977888 |
| 128.000 | MozJPEG + DQT | 127.878 | 1024×768 | 0,996608 | 0,995960 | 0,942432 |
| 128.000 | jpegli | 125.925 | 1024×768 | 0,996114 | 0,996320 | 0,980938 |

jpegli mempunyai SSIM ROI teks kecil tertinggi pada kedua target di antara ketiga jalur tanpa filter. Namun recall ambang dan SSIM dapat berbeda peringkat: MozJPEG/DQT mempunyai SSIM tinggi dengan recall foreground lebih rendah. Karena itu angka ini tidak membenarkan klaim bahwa teks pasti lebih terbaca. Pada target64KB, perubahan dimensi juga berpengaruh.

| Target gradien | Metode | Bytes | Dimensi | SSIM seluruh | PSNR RGB (dB) |
| --- | --- | ---: | --- | ---: | ---: |
| 8.000 | Turbo444 | 7.794 | 666×500 | 0,992389 | 40,056 |
| 8.000 | MozJPEG + DQT | 7.896 | 1024×768 | 0,996089 | 48,618 |
| 8.000 | jpegli | 7.993 | 1024×768 | 0,996291 | 47,244 |
| 16.000 | Turbo444 | 15.007 | 1024×768 | 0,997101 | 50,101 |
| 16.000 | MozJPEG + DQT | 15.480 | 1024×768 | 0,998109 | 52,894 |
| 16.000 | jpegli | 15.273 | 1024×768 | 0,998684 | 53,773 |

Keluaran gradien jpegli/MozJPEG mempertahankan resolusi pada8KB; Turbo membutuhkan resize. CSV juga mencakup seluruh ROI dan varian rate-coupled. Efek filter pada fixture dapat bercampur, sedangkan corpus13 sumber menunjukkan regresi SSIM pada10/13 pasangan MozJPEG dan13/13 jpegli. Bukti itu belum mendukung aktivasi filter secara default.

Inspeksi dasar dilakukan pada JPEG dokumen64KB/jpegli dan gradien8KB/jpegli. Ini pemeriksaan tampilan hasil, tanpa eksperimen manusia terkontrol. Referensi gradien sendiri delapan bit; SSIM/PSNR/MAE tidak menjamin bebas banding. Pengujian belum mencakup foto dokumen, huruf non-Latin, OCR, atau noise pemindaian.

Artefak lokal: `samples/results/jpeg-fixtures-2026-11-03/` berisi dua PNG referensi dan20 JPEG. Data versioned: [80 skor ROI](2026-11-03-jpeg-validation/fixtures/roi-quality.csv), [20 catatan encode/hash](2026-11-03-jpeg-validation/fixtures/runs.json), [environment asli](2026-11-03-jpeg-validation/fixtures/environment.json).

## Waktu dan peak RSS proses CLI

Tiga kasus mewakili foto6,29MP (`pexels-markusspiske-105003.jpg`), ilustrasi/noise3,69MP (`sample-1mb.jpg`) dan dokumen0,79MP. Target masing-masing200KB,200KB dan64KB. Tiga backend tanpa prefilter memakai log-secant eksplisit, satu warmup per kasus/metode dan tiga pengulangan terukur: **27 baris ukur + sembilan warmup =36 JPEG**. Metode dirotasi urutannya, proses diberi affinity satu CPU (`[0]`), dan keluaran tiga pengulangan identik per kelompok.

Setiap percobaan memakai helper baru yang hanya mempunyai satu anak CLI. `RUSAGE_CHILDREN.ru_maxrss` Linux mencatat peak RSS anak tersebut dalam KiB, sehingga nilainya tidak menumpuk dari proses sebelumnya. Timer mencakup peluncuran/loading, baca, kompresi, validasi, tulis dan fsync CLI; evaluasi kualitas tidak masuk timer/RSS. Tes64MiB lalu1KiB memeriksa bahwa pembacaan peak RSS memang terpisah.

| Kasus | Metode | Median waktu (s) | Min…maks (s) | Median peak RSS (KiB) | Maks peak RSS (KiB) | Dimensi hasil | Encode |
| --- | --- | ---: | ---: | ---: | ---: | --- | ---: |
| Foto | Turbo444 | 3,421 | 2,014…4,030 | 100.564 | 100.628 | 2074×1383 | 8 |
| Foto | MozJPEG + DQT | 15,494 | 10,617…17,622 | 125.160 | 125.264 | 2651×1767 | 8 |
| Foto | jpegli | 4,623 | 2,973…5,477 | 111.448 | 111.508 | 2330×1553 | 11 |
| Noise | Turbo444 | 3,547 | 3,470…3,700 | 86.300 | 86.312 | 2304×1296 | 10 |
| Noise | MozJPEG + DQT | 10,922 | 9,054…12,307 | 69.392 | 69.592 | 2560×1440 | 8 |
| Noise | jpegli | 1,667 | 1,565…1,668 | 51.156 | 51.204 | 2560×1440 | 6 |
| Teks | Turbo444 | 0,385 | 0,339…0,434 | 21.048 | 21.312 | 881×661 | 10 |
| Teks | MozJPEG + DQT | 1,860 | 1,597…2,009 | 21.876 | 22.032 | 921×691 | 8 |
| Teks | jpegli | 0,401 | 0,393…0,463 | 21.844 | 21.908 | 898×674 | 10 |

Angka ini membandingkan pipeline target-byte pada hasil yang dapat berbeda dimensi/quality, bukan throughput encoder pada piksel identik. Build host mengaktifkan semua backend tetapi setiap proses menggunakan satu backend terpilih. Host bukan mesin benchmark khusus: frekuensi CPU dan beban OS tidak dikendalikan, hanya tiga pengulangan, serta variasi waktu foto cukup besar. Tidak ada klaim speed-up umum atau latency Android. Tidak ada kompilasi/penilaian SSIM yang dijalankan bersamaan oleh tugas ini selama pengukuran performa.

Data: [27 pengukuran termasuk CPU time/hash](2026-11-03-jpeg-validation/performance/measurements.csv), [median/rentang](2026-11-03-jpeg-validation/performance/summary.json), [environment/affinity/input](2026-11-03-jpeg-validation/performance/environment.json). Semua keluaran tetap lokal di `samples/results/jpeg-performance-2026-11-03/`. Peak RSS ini menggantikan kebutuhan membagi RSS seluruh harness pada laporan lama; estimasi resource core tetap bukan hard RSS cap.

## Android ARM64

Build release **default backend + CLI** berhasil untuk `aarch64-linux-android`, Android API24, NDK r27d **27.3.13750724**, Rust1.93.0. Artefak lokal `target/aarch64-linux-android/release/dt-img` berukuran **2.351.344 bytes**. Inspeksi ELF menunjukkan ELF64/AArch64 PIE, interpreter `/system/bin/linker64`, dan dependency dinamis hanya `libdl.so`, `libm.so`, `libc.so`; TurboJPEG dibundel statis, tanpa dependency `libturbojpeg.so`.

```sh
bash tools/check_android.sh /home/arissupriy/android-sdk/ndk/ndk-r27d
```

Jalankan dari root crate dengan Rust target dan NDK tersedia. Script memilih compiler ARM64/API24, CMake toolchain NDK dan static vendor TurboJPEG. [Bukti inspeksi/hash/build](2026-11-03-jpeg-validation/android/build-evidence.json) menyimpan versi, perintah dan readelf. Build awal4m33s termasuk kompilasi, bukan waktu kompresi.

`adb devices` tidak menampilkan perangkat. **Belum ada eksekusi Android**, pengukuran memori/thermal/lifecycle, atau bridge Flutter. MozJPEG pada integrasi ini masih dibatasi Linux/shared; wrapper jpegli menolak cross-compilation. Keberhasilan default ARM64 tidak memperluas dukungan kedua backend opsional tersebut.

## Keputusan konfigurasi dan gate berikutnya

| Penggunaan | Konfigurasi yang direkomendasikan sekarang | Dasar dan batas |
| --- | --- | --- |
| Default core/CLI, calon jalur Android | Turbo444 + log-secant; filter/DQT mati | Portabel; cross-build ARM64 terbukti; biaya foto/teks lebih rendah pada sampel host. Runtime Android tetap perlu diuji. |
| Opsi kualitas desktop yang mendukung build native | jpegli/native AQ + log-secant; tanpa prefilter | SSIM tertinggi rata-rata,11/13 menang vs Turbo; ROI teks kecil kuat. Ada regresi dan keterbatasan distribusi/cross-build. |
| Pembandingan MSE/riset | MozJPEG, DQT dipilih eksplisit, log-secant | Keuntungan PSNR/DQT pada corpus lama tidak universal menurut SSIM; waktu lebih tinggi pada ketiga kasus. |
| Eksperimen filter | Tensor tetap/rate-coupled opt-in | Belum lolos gate manfaat corpus/ROI/manusia; tidak dipromosikan menjadi default. |

Ini rekomendasi atas opsi API/CLI yang sudah ada pada slice validasi ini. Tidak ada perubahan solver/encoder Rust, preset baru, atau auto-selection. Arahan pengguna sesudah pengukuran: **tunda Android, fokus perbaikan engine dan Linux**. Validasi build Android di atas dipertahankan sebagai bukti historis; tidak ada pekerjaan runtime/Flutter lanjutan sekarang. Prioritas berikutnya adalah perbandingan kualitas/ukuran/biaya engine Linux, termasuk chroma setting yang diekspos backend. T07 tetap memerlukan corpus tuning/evaluasi terpisah, OCR/penilaian manusia pada dokumen nyata dan validasi distribusi desktop.

## Reproduksi dan provenance

Build host all-feature dengan shared vendor TurboJPEG seperti [protokol native](../benchmark-protocol.md). Pilih direktori `libturbojpeg.so.0` hasil build yang sama; contoh path run ini:

```sh
VALIDATION_LIB_DIR=target/release/build/turbojpeg-sys-e0f736f7debad7b0/out/lib
python3 tools/jpeg_validation.py fixtures --library-dir "$VALIDATION_LIB_DIR" \
  --output samples/results/jpeg-fixtures-repeat
python3 tools/jpeg_validation.py performance --library-dir "$VALIDATION_LIB_DIR" \
  --fixtures samples/results/jpeg-fixtures-repeat \
  --output samples/results/jpeg-performance-repeat
```

Direktori output harus baru; hash komponen build bisa berbeda di mesin lain. Fixture membutuhkan dependency Python yang dipin pada `tools/requirements-evaluation.txt` dan font DejaVuSans atau `--font` eksplisit. Pengukuran worker sendiri hanya mengimpor stdlib, agar dependency metrik tidak masuk proses ukur.

Environment fixture asli merekam hash runner sebelum tiga baris affinity ditambahkan khusus pada `performance_run`. Environment performa merekam snapshot sesudahnya. Kedua snapshot yang benar-benar cocok dengan hash run disimpan di [runner fixture](2026-11-03-jpeg-validation/snapshots/jpeg_validation-fixtures.py) dan [runner performa](2026-11-03-jpeg-validation/snapshots/jpeg_validation-performance.py). Perubahan tersebut tidak menyentuh jalur fixture; hasil tidak di-encode ulang.

Metadata awal fixture belum merekam versi dependency dan definisi/hash scorer. [Suplemen verifikasi setelah run](2026-11-03-jpeg-validation/fixtures/metric-provenance.json) menyatakan waktu/sifat pencatatannya secara eksplisit: hash scorer masih sama dengan run104 output, versi library terpasang dicocokkan, dan hash font/referensi diverifikasi. Suplemen itu tidak diakui sebagai metadata yang ditangkap saat run. Runner terbaru kini merekam field metrik langsung untuk run berikutnya; data lama tetap dipertahankan.

Delapan unit test Python meliputi lima tes metrik dan tiga tes worker/fixture. CI ditambah job tes evaluasi; eksekusi CI remote belum dilakukan. Tidak ada dependency produksi Rust baru atau foto yang dimasukkan ke Git.
