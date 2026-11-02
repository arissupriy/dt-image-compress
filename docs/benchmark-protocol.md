# Protokol benchmark awal

Jalankan `python3 tools/benchmark.py --iterations 5 --output <direktori-baru>` dari repo. Hanya Python standard library dan toolchain Rust/C yang diperlukan.

## Yang diukur

- Empat gambar RGB sintetis 512×384: dokumen pola glyph, gradient, sensor-noise, dan fine-texture; satu PNG RGBA sintetis 128×128 dengan RGB tersembunyi.
- Setiap JPEG mempunyai target sebesar output backend baseline quality 60 pada sumber yang sama. Angka quality ini hanya menentukan budget bersama, bukan disamakan dengan quality codec lain.
- Baseline dan tensor memakai input, dimensi, backend, batas quality, metadata, dan target yang sama; tidak memakai resize.
- Satu warm-up per kombinasi JPEG, lalu lima repetisi default. Median dan p95 empiris dilaporkan; p95 dari sampel kecil tidak dianggap estimasi populasi yang kuat.
- Waktu mencakup core: inspeksi, decode, preprocessing, pencarian quality, encode, dan validasi output. Perhitungan metrik dilakukan di luar timer.
- RGB PSNR mengukur error terhadap sumber sintetis. `text_pixel_psnr` hanya memakai piksel gelap pada fixture dokumen, bukan OCR atau pembacaan karakter.
- PNG dicek equality RGBA, termasuk RGB pada alpha nol. PNG 16-bit tercakup oleh suite test terpisah.
- Linux `/proc/self/status` bila tersedia mencatat `VmHWM`, peak RSS proses benchmark itu sendiri, tanpa proses kompilasi. Angka tersebut bukan peak per gambar; environment.json menyatakan apakah pengukuran tersedia.

CSV, summary.json, environment.json, hash input, dan gambar before/after disimpan bersama. Jika kompresi gagal atau melampaui target, harness gagal; jangan memilih hanya hasil yang menguntungkan.

## JPEG lokal pengguna

```sh
python3 tools/benchmark.py --samples samples --iterations 3 --output target/jpeg-samples-run
```

- Tiga budget desimal: 200.000, 500.000, dan 1.000.000 bytes. Quality 40–95 dan backend yang sama untuk semua metode.
- B0: quality search pada resolusi asli. B1-resize: B0 dengan izin resize Lanczos3, long edge minimum 1.024 piksel. E1-tensor: B0 ditambah tensor threshold 0,04/coherence boost 4. E1-tensor-resize: E1 dengan izin resize yang sama.
- Izin resize tidak berarti resize selalu terjadi. CSV menyimpan dimensi aktual, flag resized, jumlah encode, dan quality. Seluruh percobaan dibatasi 32 encode. Filter dijalankan pada sumber yang dinormalisasi sebelum pencarian skala; threshold dan blur tetap untuk seluruh pencarian.
- Satu warm-up dan tiga repetisi untuk setiap kombinasi sampel/metode/budget. `TargetUnreachable` dicatat beserta ukuran terkecil yang sempat diamati, bukan dipalsukan sebagai output sukses. Error lain menghentikan harness.
- PSNR RGB dihitung terhadap piksel JPEG asli yang didecode independen oleh `image`, setelah orientasi EXIF dinormalisasi. Hasil resize dipulihkan ke dimensi referensi memakai Lanczos3 sebelum penghitungan; ini memasukkan kehilangan detail akibat resize. Metrik tersebut bukan OCR, SSIM, pengujian profil warna, atau penilaian manusia. Nilai quality dari input JPEG tidak diketahui, dan JPEG asli bukan ground truth RAW/lossless.
- Sampel diberi alias `sample-01` dan seterusnya menurut urutan nama file. `environment.json` menyimpan peta nama/ukuran/hash; file asli tidak disalin, tidak diubah, tidak diunggah, dan tidak disertakan dalam paket crate. Output gambar berada dalam direktori hasil lokal.
- VmHWM mencakup seluruh proses benchmark, termasuk referensi/metrik di luar timer core. Angka ini bukan hard cap alokasi core atau RSS per gambar.

Corpus pengguna ini melengkapi smoke test; belum menggantikan corpus evaluasi berlisensi yang beragam. B2 (denoise sederhana), E2 (rate coupling), dan E3 (backend alternatif) masih perlu diuji sebelum menyimpulkan manfaat gabungan secara umum.

## Ablation JPEG teroptimasi pada corpus tambahan

```sh
python3 tools/benchmark.py --samples samples --jpeg-suite --iterations 1 --output samples/results/jpeg-suite-run
```

Mode ini menjalankan tujuh metode pada budget 200.000 bytes: B0-image; E3-turbo444; E3-turbo420; B2-gaussian (Turbo444); E1-fixed (Turbo444); E2-rate (Turbo444); E2-rate420. Semua mengizinkan resize Lanczos3, minimum long edge1.024; setiap metode dapat memilih dimensi berbeda. Chroma420 merupakan perubahan representasi lossy yang dilaporkan, bukan penghematan entropy murni.

H2 mengukur referensi q80 tanpa filter pada backend yang dipilih dan menghitung probe dalam budget32 encode. `tau=min(max(tau_base,.25),tau_base*sqrt(max(1,B_q80/B_target)))`. Threshold dicatat di CSV, dibekukan selama quality/skala search, dan dibatasi. B2 memakai Gaussian sigma .8 tanpa tensor. E1 memakai threshold .04 dan coherence boost4.

Corpus foto resolusi asli mencakup input di atas20MP. Mode desktop ini menaikkan limit eksplisit menjadi50MP/1GiB; default library20MP/256MiB tetap berlaku. Tidak ada resize tersembunyi sebelum core. Referensi metrik mengikuti orientasi EXIF, dan hasil resize dipulihkan ke dimensi sumber sebelum PSNR.

Satu repetisi tanpa warmup dipakai untuk perbandingan ukuran/kualitas deterministik; waktu yang ikut tercatat bukan dasar klaim latency atau speed-up. Jalankan repetisi/perangkat terisolasi jika performa menjadi objek penelitian. Mode ini mengisi ablation B2/E2/E3 yang belum ada pada snapshot awal; hasil positif dan regresi harus dilaporkan bersama.

## Ablation native JPEG, DQT, dan log-secant

```sh
python3 tools/benchmark.py --samples samples --jpeg-native-suite --iterations 1 --output samples/results/jpeg-native-run
```

Linux/Rust>=1.90 diperlukan untuk ketiga backend dalam satu proses. Script mengaktifkan feature `experimental-prefilter,mozjpeg,jpegli`, memaksa shared vendor TurboJPEG untuk mencegah konflik simbol, lalu memakai `cargo run` agar runtime memuat library hasil build yang tepat.

Delapan metode pada target200.000 bytes dan limit desktop yang sama:

| Metode | Encoder444 | Pencarian | DQT/AQ | Prefilter |
| --- | --- | --- | --- | --- |
| T0-turbo-binary | TurboJPEG | Binary | Standar | Tidak |
| T1-turbo-log | TurboJPEG | Log-secant | Standar | Tidak |
| M0-moz-binary | MozJPEG | Binary | Q_OPT mati | Tidak |
| M1-moz-log | MozJPEG | Log-secant | Q_OPT mati | Tidak |
| M2-moz-dqt-log | MozJPEG | Log-secant | Q_OPT aktif | Tidak |
| J0-jpegli-log | Google jpegli | Log-secant | AQ native aktif | Tidak |
| M3-moz-dqt-rate-log | MozJPEG | Log-secant | Q_OPT aktif | Rate-coupled tensor |
| J1-jpegli-rate-log | Google jpegli | Log-secant | AQ native aktif | Rate-coupled tensor |

Bandingkan T1/T0 dan M1/M0 untuk pengaruh pencarian (kualitas, ukuran, dimensi, probe); M2/M1 untuk DQT; M3/M2 dan J1/J0 untuk prefilter; serta encoder pada budget identik. Encoder memiliki skala quality berbeda. Semua metode memakai satu encoder per percobaan, resize hanya dengan izin suite, dan batas32 encode termasuk probe filter. Pemilihan dimensi dapat berbeda, sehingga perbandingan memasukkan dampak resize. Satu run tanpa warmup menilai keluaran deterministik dan jumlah probe, bukan keunggulan waktu atau memori mobile.

## Gate sebelum mengubah default

Tambahkan corpus riil berlisensi, pisahkan tuning/evaluasi, gunakan metrik ROI dan pemeriksaan manusia, ukur pada perangkat sasaran, dan bandingkan denoise sederhana/backend alternatif. Data sintetis tidak membenarkan klaim penghematan umum, text preservation, atau latency mobile. Feature eksperimen tetap mati secara default sampai pengujian tersebut selesai.
