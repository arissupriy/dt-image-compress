# dt-image-compress

Mesin kompresi gambar offline untuk DailyTools dan aplikasi Rust lain. Source ini disiapkan untuk repositori mandiri [arissupriy/dt-image-compress](https://github.com/arissupriy/dt-image-compress); belum dipublikasikan ke crates.io.

## Dukungan saat ini

| Input → output | Dukungan |
| --- | --- |
| JPEG → JPEG | RGB/YCbCr/grayscale 8-bit; quality 40–95, target bytes, normalisasi EXIF orientation 1–8, resize opsional. CMYK/YCCK ditolak sebagai belum didukung. |
| PNG → PNG | Optimasi lossless yang mempertahankan sampel, termasuk RGB di bawah alpha nol dan 16-bit. |
| PNG 8-bit → JPEG | Konversi lossy; transparansi membutuhkan warna latar eksplisit. Metadata warna/orientasi kompleks ditolak bila belum dapat ditangani dengan benar. |
| WebP, GIF/APNG, SVG, AVIF, HEIC | Belum didukung; tidak diproses diam-diam sebagai frame pertama atau format lain. |

Semua hasil sukses dengan target memenuhi `output_bytes <= target_bytes`. Jika tidak tercapai dalam batas kualitas, dimensi, dan jumlah encode, hasilnya `TargetUnreachable`, tanpa output sukses yang terlalu besar. Solver mengembalikan kandidat feasible terbaik yang diukur, bukan jaminan optimum global terhadap semua setting encoder.

Pencarian quality dapat dipilih dengan `--search binary` (default) atau `--search log-secant`, serta `CompressionOptions::target_search` di library. Log-secant mengukur endpoint quality 95/40, menginterpolasi log ukuran di dalam bracket feasible/infeasible, dan beralih ke midpoint saat konvergensi lambat atau interpolasi tidak valid. Kedua metode menjaga hard cap dan budget encode; keduanya memakai asumsi ukuran yang kira-kira monoton terhadap quality. Log-secant tidak mengubah piksel atau encoder.

## Menjalankan CLI

MSRV deklaratif Rust 1.88, dengan pengujian CI disiapkan untuk memverifikasinya. Build memerlukan C toolchain dan CMake untuk dependency native libdeflate serta libjpeg-turbo. Pengujian lokal pertama menggunakan Rust 1.93.0 di Linux x86_64; MSRV dan platform lain belum diverifikasi secara lokal.

```sh
cargo run --locked --release --features cli -- photo.jpg -o smaller.jpg --target 200KB
cargo run --locked --release --features cli -- photo.jpg -o smaller.jpg --target 200KiB --allow-resize
cargo run --locked --release --features cli -- signature.png -o optimized.png
cargo run --locked --release --features cli -- signature.png -o signature.jpg --format jpeg --background FFFFFF
cargo run --locked --release --features cli -- - -o - --target 200KB < photo.jpg > smaller.jpg
```

- `KB` = 1.000 bytes; `KiB` = 1.024 bytes. Gunakan unit dengan kapitalisasi yang ditampilkan. Nilai nol, pecahan, unit ambigu, dan overflow ditolak.
- Tidak ada resize otomatis tanpa `--allow-resize`; batas long edge default 1.024 piksel atau ukuran asli jika lebih kecil. Gunakan `--min-long-edge` untuk memilih batas lain.
- Tanpa target, JPEG diencode satu kali pada quality 80 tanpa resize; PNG menjalankan optimasi lossless.
- Default CLI adalah lossy untuk JPEG dan lossless untuk PNG. Library meminta caller memilih mode PNG secara eksplisit.
- File output existing tidak pernah ditimpa. Penyimpanan file menggunakan temporary file dan publish tanpa clobber setelah kompresi selesai. Interrupt Ctrl+C diterjemahkan menjadi cancel kooperatif.
- Bytes gambar saja masuk stdout jika `-o -`; report/error masuk stderr. Pipe tidak memiliki jaminan atomic file, termasuk ketika shell membuat atau menimpa tujuan redirection.

## Library

```rust
use dt_image_compress::{compress, CompressionOptions};
use std::ops::ControlFlow;

# fn example(input: &[u8]) -> Result<(), dt_image_compress::CompressionError> {
let options = CompressionOptions {
    target_bytes: Some(200_000),
    ..Default::default()
};
let output = compress(input, &options, |event| {
    // Return Break(()) to cancel at the next checkpoint.
    let _ = event;
    ControlFlow::Continue(())
})?;
assert!(output.bytes.len() <= 200_000);
# Ok(())
# }
```

Core menerima bytes dan mengembalikan bytes/report; tidak menulis file, memerlukan Flutter, atau menjalankan request jaringan. Feature `cli` hanya menambahkan executable dan dependency terminal.

Metadata default `RemovePrivate` menghapus metadata privat sambil mempertahankan informasi warna yang didukung. Ini bukan penghapusan informasi sensitif dalam isi gambar. PNG lossless dengan eXIf saat ini harus memakai `Preserve` atau ditolak; JPEG recompression dengan `Preserve` belum didukung. ICC RGB dipertahankan pada JPEG; profil non-RGB dan konversi high-bit-depth yang belum aman ditolak.

Decode piksel JPEG memakai libjpeg-turbo dan memperlakukan peringatan decode sebagai error, karena decoder Rust yang diuji menerima beberapa stream entropy terpotong tanpa error. Metadata/header dan validasi output tetap memakai `image`. Encoder default kini libjpeg-turbo dengan optimized Huffman dan progressive, 4:4:4 untuk mempertahankan sampel chroma. `--jpeg-backend turbo420` mengizinkan chroma subsampling 4:2:0; `--jpeg-backend image` menyediakan baseline lama untuk pembandingan. ICC RGB tetap disalin melalui marker APP2. Progressive input dibatasi hingga 100 scan.

Batas awal: input 32 MiB, 20 juta piksel, estimasi working memory 256 MiB, maksimal 32 encode. Jumlah piksel yang dapat diproses juga bergantung pada bit depth dan buffer. Limit decode serta estimasi buffer bukan hard cap RSS untuk semua alokasi codec. Cancel diperiksa antar-tahap/percobaan; panggilan encoder yang sedang berjalan tidak dipotong paksa.

## Eksperimen algoritma

Feature `experimental-prefilter` menambahkan prefilter struktur lokal yang **mati secara default**. Filter memakai tile 256×256 dengan halo 2 piksel dan mengikuti batas input/budget core: default 20 megapiksel, batas atas filter 50 megapiksel bila caller menaikkan budget secara eksplisit. API diagnostik `experimental::analyze` yang mengembalikan seluruh peta tensor tetap dibatasi 1 megapiksel. Implementasinya memakai Scharr, Gaussian window pada ketiga komponen tensor, koherensi, dan convex blend dengan citra blur. Kanal berbobot sRGB dipakai sebagai luma proxy nonlinear; ini bukan OCR, deteksi wajah, atau jaminan bahwa teks tetap terbaca.

`--prefilter` memakai threshold tetap. `--adaptive-prefilter` menjalankan H2: satu probe JPEG tanpa filter pada quality 80 mengukur tekanan `rho=max(1,reference_bytes/target_bytes)`, lalu `tau=min(max(base_tau,0.25),base_tau*sqrt(rho))`. Filter dibekukan selama pencarian quality/skala; probe dihitung dalam budget 32 encode. Jika referensi q80 sudah feasible dan budget sisanya gagal menghasilkan kandidat terfilter, referensi dipertahankan. Report menyimpan backend dan threshold efektif. Rumus ini merupakan hipotesis eksperimen yang diimplementasikan, bukan klaim optimalitas. Gaussian blur sederhana juga tersedia sebagai kontrol di API eksperimen.

```sh
cargo run --locked --release --features cli,experimental-prefilter -- photo.jpg -o experiment.jpg --target 200KB --prefilter
cargo run --locked --release --features cli,experimental-prefilter -- photo.jpg -o adaptive.jpg --target 200KB --adaptive-prefilter
python3 tools/benchmark.py --self-test
python3 tools/benchmark.py --iterations 5 --output benchmark-results
python3 tools/benchmark.py --samples samples --iterations 3 --output target/jpeg-samples-run
```

Direktori output benchmark harus belum ada. Harness membandingkan baseline dan tensor pada byte budget yang sama dengan lima fixture sintetis, menyimpan input/output, CSV, manifest hash, lingkungan, dan ringkasan. Dataset ini untuk smoke test, bukan pembuktian keunggulan atau novelty. [Protokol benchmark](docs/benchmark-protocol.md) menjelaskan batas metrik dan langkah berikutnya.

Mode `--samples` menguji JPEG lokal pada 200 KB, 500 KB, dan 1 MB, tanpa/dengan tensor dan tanpa/dengan izin resize. File asli tidak disalin ke output. `samples/` diabaikan Git dan dikecualikan dari paket crate. CSV juga menyimpan kasus target tidak tercapai; hasil resize mempunyai dimensi tersendiri dan tidak boleh dianggap sebagai keuntungan filter saja.

[Laporan tiga JPEG pengguna](docs/benchmarks/jpeg-samples-2026-11-02.md) menyimpan snapshot sebelum encoder default dan H2 ditingkatkan. Manfaat filter harus dinilai ulang terhadap backend teroptimasi; tidak otomatis menjadi default.

[Laporan13 JPEG/tujuh metode](docs/benchmarks/jpeg-suite-2026-11-02.md):91/91 keluaran memenuhi200KB. Encoder teroptimasi444 memberi PSNR lebih tinggi pada12/13 gambar vs baseline; manfaat coupling tetap bercampur. Jalankan `python3 tools/benchmark.py --samples samples --jpeg-suite --iterations 1 --output samples/results/jpeg-suite-run` untuk mereproduksi dengan profil desktop50MP/1GiB. Output berada di `samples/results/`, file asli tidak diubah.

## Verifikasi

```sh
cargo fmt --check
cargo test --locked
cargo test --locked --all-features --all-targets
cargo clippy --locked --all-targets --all-features -- -D warnings
python3 tools/benchmark.py --self-test
```

## Status dan lisensi

Lihat [catatan implementasi](docs/implementation-log.md), [audit dependency](docs/dependency-audit.md), dan [arah riset](docs/research.md). Lisensi package yang diusulkan mengikuti rencana: `MIT OR Apache-2.0`. Publikasi dan keputusan distribusi final tetap mengikuti pemilik proyek. Tidak ada repo induk/workspace bersama yang diwajibkan oleh crate ini.
