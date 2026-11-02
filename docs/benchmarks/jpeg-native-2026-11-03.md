# JPEG native: MozJPEG, adaptive DQT, jpegli, log-secant

Run berlangsung 2–3 November 2026 di Linux x86_64/Rust 1.93.0. Ada 13 JPEG pengguna, delapan metode, dan target 200.000 bytes. **104/104 sukses**, seluruh output <=target dan jumlah encode <=32. Semua 13 hash sumber sama dengan run sebelumnya; file asli tidak berubah. JPEG berada di `samples/results/jpeg-native-2026-11-02/` (nama folder mengikuti tanggal mulai).

## Konfigurasi dan integritas

Snapshot kode benchmark: **d9186d2**, sebelum promosi log-secant menjadi default. Seluruh hash Rust/Cargo/build dalam environment.json cocok dengan snapshot Git tersebut. Tidak ada perubahan algoritma atau konfigurasi di tengah run. Metode pencarian setiap baris dipilih eksplisit; perubahan default setelah pengukuran tidak mengubah konfigurasi yang diuji. Suite lama kini juga menetapkan binary secara eksplisit untuk menjaga reproduksinya.

- TurboJPEG menggunakan optimized Huffman/progressive444. MozJPEG melalui mozjpeg-sys2.2.3 menggunakan native maximum-compression profile, trellis AC/DC, optimized Huffman dan pencarian scan progresif. SIMD native aktif. Q_OPT hanya aktif pada metode berlabel dqt.
- DQT adaptif memakai `JBOOLEAN_TRELLIS_Q_OPT`, bukan tabel tetap yang diberi nama adaptif. Pada semua 13 pasangan M2/M1, marker DQT akhir berbeda. Unit test pada quality yang sama juga membuktikan pengaruh isi piksel terhadap tabel.
- Google jpegli asli melalui jpegli-rust0.1.1, native AQ aktif, progressive444. AQ jpegli berbeda dari optimasi tabel MozJPEG; wrapper tidak menyediakan toggle AQ.
- T0/M0 memakai binary; enam metode lainnya log-secant. Filter rate-coupled memakai probe q80 yang dihitung dalam budget, threshold dibekukan selama pencarian. Setiap percobaan memakai satu encoder.
- Semua metode boleh resize secara eksplisit, minimum long edge 1.024, quality 40–95. Skala quality antarencoder tidak ekuivalen. Profil desktop 50MP/1GiB dipilih untuk foto sampai sekitar 40MP; default core tetap 20MP/256MiB. Tidak ada downscale sebelum core.
- Static TurboJPEG/MozJPEG terbukti berbenturan simbol pada probe. Run ini memakai vendor shared TurboJPEG dan static MozJPEG serta prefixed static jpegli. `/proc` proses benchmark mengonfirmasi library yang dimuat adalah `target/release/build/turbojpeg-sys-e0f736f7debad7b0/out/lib/libturbojpeg.so.0.4.0`, bukan library sistem. Build guard mencegah konfigurasi yang belum aman.

Referensi metrik: decode independen JPEG asli, EXIF dinormalisasi. Hasil resize dipulihkan ke dimensi referensi dengan Lanczos3 sebelum RGB PSNR. Ini menilai keseluruhan pipeline termasuk resize, bukan isolasi kualitas encoder pada resolusi identik. JPEG sumber bukan RAW/lossless ground truth. Satu run tanpa warmup dan aktivitas build/test lain tidak mendukung klaim latency/speed-up. Peak RSS seluruh proses **897.116KiB / 876,09MiB**, termasuk referensi/metrik, bukan hard cap atau pengukuran per gambar/mobile.

## Hasil delapan metode

| Metode | Konfigurasi | Rata-rata PSNR RGB (dB) | Total encode13 sampel | Sampel di-resize |
| --- | --- | ---: | ---: | ---: |
| T0-turbo-binary | Turbo444 + binary |30,7997|223|13|
| T1-turbo-log | Turbo444 + log-secant |30,7997|138|13|
| M0-moz-binary | MozJPEG444 + binary |30,9199|210|11|
| M1-moz-log | MozJPEG444 + log-secant |30,9199|133|11|
| M2-moz-dqt-log | MozJPEG444 + DQT + log-secant |30,9310|133|11|
| J0-jpegli-log | jpegli444/native AQ + log-secant |30,9302|131|12|
| M3-moz-dqt-rate-log | Tensor adaptif + MozJPEG444/DQT + log-secant |30,9244|136|11|
| J1-jpegli-rate-log | Tensor adaptif + jpegli444/native AQ + log-secant |30,8725|146|12|

## Apa yang memberi manfaat

| Pasangan | PSNR naik / sama / turun | Rata-rata delta (dB) | Rentang delta (dB) |
| --- | ---: | ---: | ---: |
| MozJPEG vs Turbo, keduanya log-secant |12/0/1|+0,1202|−0,1019…+0,5617|
| jpegli vs Turbo, keduanya log-secant |9/0/4|+0,1305|−0,2948…+1,1813|
| DQT adaptif vs DQT tetap pada MozJPEG |12/0/1|+0,0111|−0,0002…+0,0344|
| Tambah tensor adaptif pada MozJPEG/DQT |3/0/10|−0,0066|−0,1479…+0,3315|
| Tambah tensor adaptif pada jpegli |3/0/10|−0,0577|−0,3791…+0,0343|

**Log-secant:** semua 13 output Turbo dan semua 13 output MozJPEG byte-identik dengan binary. Total encode turun 223→138 (**38,12%**) dan 210→133 (**36,67%**). Turbo: 12 sampel lebih sedikit probe, 1 sama. MozJPEG: 11 lebih sedikit, 1 sama, 1 lebih banyak; pada sample-1mb, 8 vs 7. Ini pengurangan percobaan, bukan klaim waktu turun sebesar persentase yang sama. Berdasarkan hasil ini log-secant menjadi default; binary tetap tersedia untuk kontrol. Kedua solver tetap mengandalkan ukuran yang kira-kira monoton, tanpa jaminan optimum global pada seluruh parameter.

**Encoder/DQT:** keuntungan MozJPEG cukup konsisten pada corpus ini; DQT memberi tambahan kecil dan tidak universal. jpegli mempunyai rata-rata tinggi tetapi empat regresi terhadap Turbo. Angka quality dan dimensi yang lebih besar tidak dipakai sebagai bukti fidelity tersendiri. Seluruh output T0 dan nilai PSNR-nya juga identik dengan E3-turbo444 pada run lama: penambahan native backend tidak mengubah hasil encoder lama.

**Filter:** gabungan benar-benar berjalan, tetapi filter merugikan PSNR pada 10/13 gambar untuk masing-masing backend baru. Filter tetap opt-in; menambahkan algoritma tidak otomatis memperbaiki hasil.

## Contoh sumber1MB

`sample-1mb.jpg`: 1.022.240 bytes, 2560×1440.

| Jalur | Bytes | Dimensi | PSNR RGB | Quality | Encode |
| --- | ---: | --- | ---: | ---: | ---: |
| Turbo444 + log |196.956|2304×1296|30,2335|51|10|
| MozJPEG + log |196.569|2560×1440|30,3562|59|8|
| MozJPEG + DQT + log |196.569|2560×1440|30,3906|59|8|
| jpegli/native AQ + log |191.323|2560×1440|30,4627|59|6|
| Tensor adaptif + MozJPEG/DQT + log |190.047|2560×1440|30,2427|67|9|
| Tensor adaptif + jpegli/native AQ + log |198.643|2560×1440|30,2887|73|12|

Jalur jpegli mengurangi ukuran **81,28% tanpa resize** pada contoh ini. Ini tetap lossy; PSNR dibanding decode sumber JPEG, bukan jaminan kualitas identik. Filter dapat menaikkan angka quality sambil menurunkan fidelity, sebagaimana contoh di atas.

## Pilihan hasil untuk inspeksi

`samples/results/best-psnr-200KB-v2/` berisi satu JPEG per sumber, dipilih setelah pengukuran dari tujuh label metode lama dan delapan label baru. Tie-break memakai PSNR yang dilaporkan 4 desimal, ukuran lebih kecil, jumlah encode lebih sedikit, lalu nama metode. `selection.csv` memetakan sumber/metode/dimensi/hash output.

Pilihan akhir: 4 jpegli, 4 MozJPEG+DQT, 2 MozJPEG+DQT+tensor adaptif, 2 Turbo420, 1 image baseline. Sebanyak 10 berasal dari run native; 3 mempertahankan output lama. Dibanding pilihan terbaik run lama, 10 PSNR meningkat dan 3 tetap; rata-rata +0,0951dB. Ini **pemilihan setelah eksperimen**, bukan auto-selection di library dan bukan pembuktian superioritas perseptual. Bahkan pilihan terbaik dari delapan metode baru saja masih kalah terhadap pilihan terbaik tujuh metode lama pada 3/13 gambar.

## Reproduksi dan batas dukungan

```sh
python3 tools/benchmark.py --samples samples --jpeg-native-suite --iterations 1 --output samples/results/jpeg-native-run
```

Script mengatur shared/vendor TurboJPEG dan menjalankan binary melalui Cargo agar runtime memakai library build yang benar. Backend native ini Linux/Rust>=1.90 untuk build gabungan; integrasi Android/ARM64/Windows belum divalidasi dan wrapper jpegli tidak mendukung cross-compilation. Default backend tetap Turbo444; filter/DQT dan backend lain dipilih eksplisit. Native output buffer dibatasi, tetapi alokasi internal bukan hard RSS cap.

Data lengkap tanpa foto asli: [CSV](2026-11-03-jpeg-native/results.csv), [ringkasan](2026-11-03-jpeg-native/summary.json), [environment/hash](2026-11-03-jpeg-native/environment.json), [perhitungan/integritas](2026-11-03-jpeg-native/analysis.json), [pilihan output](2026-11-03-jpeg-native/selection.csv). Sebanyak 58 tes all-feature/all-target lulus setelah promosi default; test default, lint, format dan build release dicatat pada ledger implementasi. SSIM/OCR, penilaian manusia terkontrol, corpus terpisah untuk tuning/evaluasi dan pengujian perangkat sasaran tetap terbuka.
