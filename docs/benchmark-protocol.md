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

## Gate sebelum mengubah default

Tambahkan corpus riil berlisensi, pisahkan tuning/evaluasi, gunakan metrik ROI dan pemeriksaan manusia, ukur pada perangkat sasaran, dan bandingkan denoise sederhana/backend alternatif. Data sintetis tidak membenarkan klaim penghematan umum, text preservation, atau latency mobile. Feature eksperimen tetap mati secara default sampai pengujian tersebut selesai.
