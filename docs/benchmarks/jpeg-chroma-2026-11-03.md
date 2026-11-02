# Engine Linux: jpegli4:2:0 dan perbandingan chroma

Engine kini menyediakan **`--jpeg-backend jpegli420`**: Google jpegli/native AQ + progressive4:2:0 + pencarian log-secant. Pada corpus13 sumber, jalur ini menaikkan PSNR RGB terhadap jpegli444 pada **13/13 gambar**, rata-rata **+0,2255dB**; SSIM naik9/13 dan turun4/13. Ini perbaikan pilihan engine yang terukur, dengan kompromi warna yang tetap perlu diperhatikan.

Default tetap Turbo444; chroma dipilih eksplisit dan dibekukan selama pencarian. Android/Flutter ditunda sesuai arahan pengguna; pekerjaan ini fokus Linux.4:2:0 memakai resolusi chroma lebih rendah. API `Subsampling::S420` diverifikasi pada source dependency terkunci jpegli-rust0.1.1 dan [dokumentasi wrapper](https://github.com/oximg/jpegli-rust#api); tidak ada fork codec, algoritma baru yang diklaim, atau dependency produksi tambahan.

## Rancangan pembandingan

Empat jalur tanpa prefilter: T1 Turbo444, T2 Turbo420, J0 jpegli444/nativeAQ dan J2 jpegli420/nativeAQ. Seluruhnya log-secant, target200.000 bytes, quality40–95, min long edge1024, resize diizinkan eksplisit dan maksimum32 encode. Profil desktop50MP/1GiB menampung sumber sampai sekitar40MP; default core20MP/256MiB tetap berlaku.

Ada **52/52 keluaran sukses**, semuanya <=200.000 bytes dan <=32 encode.13 hash sumber tidak berubah. Seluruh26 kontrol T1/J0 byte-identik dengan run native sebelumnya; nilai PSNR Rust pada kontrol juga identik. Hash Rust/Cargo/build yang direkam cocok dengan snapshot **591184c**. Daftar metode suite lama dipertahankan.

SSIM dan PSNR RGB Pillow memakai [protokol evaluasi sebelumnya](jpeg-ssim-2026-11-03.md): luma nonlinear, Gaussian11×11/sigma1,5, population covariance, crop5, resolusi referensi asli. Resize keluaran dipulihkan dengan Pillow Lanczos.26 pasangan baru dihitung;26 kontrol memakai ulang skor lama hanya setelah hash sumber/JPEG, scorer dan versi dependency cocok. Tidak ada encode ulang untuk menghitung metrik. Waktu scoring135,81s adalah biaya evaluasi pada host, bukan latency engine.

## Hasil corpus

| Jalur | SSIM Y rata-rata | PSNR RGB Pillow (dB) | Total encode13 sumber | Jumlah resize |
| --- | ---: | ---: | ---: | ---: |
| T1 Turbo444 + log | 0,7256233 | 30,8027 | 138 | 13 |
| T2 Turbo420 + log | **0,7371099** | 30,7718 | 116 | 12 |
| J0 jpegli444 + log | 0,7309251 | 30,9334 | 131 | 12 |
| J2 jpegli420 + log | 0,7334942 | **31,1588** | 132 | 11 |

| Pasangan | SSIM naik/sama/turun | Delta SSIM rata-rata | PSNR naik/sama/turun | Delta PSNR rata-rata |
| --- | ---: | ---: | ---: | ---: |
| J2 vs J0 | 9/0/4 | +0,0025691 | 13/0/0 | +0,2255dB |
| J2 vs T1 | 12/0/1 | +0,0078709 | 13/0/0 | +0,3561dB |
| J2 vs T2 | 3/0/10 | −0,0036157 | 13/0/0 | +0,3870dB |
| T2 vs T1 | 13/0/0 | +0,0114866 | 7/0/6 | −0,0309dB |

Rata-rata memberi bobot sama kepada setiap sumber; tie tolerance1e−12. **Turbo420 mempunyai SSIM luma tertinggi rata-rata**, sedangkan jpegli420 mempunyai PSNR RGB tertinggi pada seluruh13 pasangan terhadap ketiga jalur lain. Bahkan dua encoder420 berbeda peringkat menurut kedua metrik. Tidak ada satu pemenang universal; SSIM luma tidak menangkap seluruh kesalahan warna. Perbedaan dimensi/quality adalah bagian dari pipeline yang dibandingkan, bukan bukti encoder pada resolusi identik.

## Contoh sumber1MB

`sample-1mb.jpg`:1.022.240 bytes,2560×1440.

| Jalur | Bytes | Dimensi | SSIM Y | PSNR RGB Pillow | Encode |
| --- | ---: | --- | ---: | ---: | ---: |
| Turbo444 | 196.956 | 2304×1296 | 0,600567 | 30,2391 | 10 |
| Turbo420 | 194.445 | 2560×1440 | 0,616441 | 30,2820 | 6 |
| jpegli444 | 191.323 | 2560×1440 | 0,608701 | 30,4691 | 6 |
| jpegli420 | **186.425** | **2560×1440** | **0,623422** | **30,6047** | **5** |

jpegli420 mengurangi ukuran81,76% dari sumber, atau2,56% lebih kecil dari jpegli444 pada contoh ini. CLI release tanpa flag pencarian memakai default log-secant dan menghasilkan bytes identik dengan J2 suite. File lokal: `samples/results/sample-1mb-200KB-jpegli420.jpg`. Ini tetap lossy, tanpa jaminan kualitas identik dengan sumber.

## Teks dan gradien: manfaat beserta regresi warna

Fixture deterministik yang sama1024×768, font/ROI/target sama, empat metode tanpa filter: **16 JPEG dan64 skor seluruh/ROI**. Semua memenuhi target. Delapan kontrol444 dan32 skor ROI identik dengan run fixture sebelumnya. Tabel menampilkan jpegli444/420; data mentah juga memuat kedua jalur Turbo.

| Fixture/target | Jalur | Bytes | Dimensi | SSIM seluruh | SSIM teks kecil | PSNR RGB seluruh |
| --- | --- | ---: | --- | ---: | ---: | ---: |
| Dokumen64KB | jpegli444 | 63.427 | 898×674 | 0,971963 | 0,961685 | 26,7280 |
| Dokumen64KB | jpegli420 | 63.326 | 921×691 | 0,974979 | 0,966023 | 26,8673 |
| Dokumen128KB | jpegli444 | 125.925 | 1024×768 | 0,996114 | 0,996320 | 35,8863 |
| Dokumen128KB | jpegli420 | 125.032 | 1024×768 | 0,996662 | 0,996904 | **33,8694** |
| Gradien8KB | jpegli444 | 7.993 | 1024×768 | 0,996291 | — | 47,2439 |
| Gradien8KB | jpegli420 | 7.984 | 1024×768 | 0,996940 | — | 46,8839 |
| Gradien16KB | jpegli444 | 15.273 | 1024×768 | 0,998684 | — | 53,7726 |
| Gradien16KB | jpegli420 | 14.212 | 1024×768 | 0,998760 | — | **47,7825** |

Dokumen128KB tidak di-resize: kenaikan SSIM420 bersamaan dengan penurunan PSNR RGB sekitar2,02dB; gradien16KB turun sekitar5,99dB. Karena itu jalur420 tetap eksplisit, terutama pada detail warna/teks. PSNR RGB dan SSIM bukan ukuran banding atau OCR; recall foreground di CSV tetap proxy ambang piksel. JPEG dokumen128KB/jpegli420 ditinjau secara visual, tanpa klaim penilaian manusia terkontrol.

## Keputusan engine Linux

- **Default tetap Turbo444 + log-secant**, tanpa filter. Tidak mengubah pilihan caller secara otomatis.
- **jpegli420 adalah opsi tambahan untuk budget ketat** dengan hasil PSNR RGB kuat pada corpus ini dan keuntungan SSIM rata-rata terhadap jpegli444. Empat regresi SSIM dan regresi warna fixture tetap dicatat.
- **Turbo420 tetap relevan:** SSIM luma naik13/13 terhadap Turbo444 dan tertinggi rata-rata dalam empat jalur ini, walaupun PSNR RGB tidak konsisten. Tidak menganggap jpegli selalu mengalahkan Turbo.
- **jpegli444 tetap tersedia** ketika resolusi chroma perlu dipertahankan; tidak ada deteksi otomatis teks atau klaim keterbacaan. MozJPEG/DQT dan prefilter tetap mengikuti hasil riset sebelumnya.

Timing corpus terjadi bersamaan dengan build/test/fixture lain, tanpa pengulangan/warmup; tidak digunakan untuk klaim speed-up. RSS seluruh harness juga bukan peak per gambar. [Profiling proses sebelumnya](jpeg-validation-2026-11-03.md) hanya mencakup jalur444/MozJPEG-DQT, sehingga latency/RAM terisolasi jalur420 belum dibandingkan. Corpus belum dipisah untuk tuning/evaluasi dan referensinya JPEG yang sudah lossy. Benchmark lintas engine eksternal serta OCR/manusia tetap terbuka.

## Reproduksi dan data

```sh
python3 tools/benchmark.py --samples samples --jpeg-chroma-suite --iterations 1 --output samples/results/jpeg-chroma-repeat
python3 docs/benchmarks/2026-11-03-jpeg-chroma/score_chroma.py samples/results/jpeg-chroma-repeat samples/results/jpeg-chroma-quality-repeat
```

Scorer studi memakai cache versioned104 output sebelumnya dan menolak perubahan corpus/scorer/dependency. Alternatif tanpa cache: gunakan `tools/jpeg_quality.py` pada run chroma; hasil protokol sama dengan menghitung seluruh52 pasangan. Dependency Python ada pada `tools/requirements-evaluation.txt`.

Data versioned: [52 hasil encode](2026-11-03-jpeg-chroma/results.csv), [environment/hash source](2026-11-03-jpeg-chroma/environment.json), [integritas dan bukti CLI](2026-11-03-jpeg-chroma/integrity.json), [52 skor kualitas](2026-11-03-jpeg-chroma/quality/quality.csv), [ringkasan/pasangan](2026-11-03-jpeg-chroma/quality/summary.json), [provenance cache/scorer](2026-11-03-jpeg-chroma/quality/environment.json), [64 skor ROI](2026-11-03-jpeg-chroma/fixtures/roi-quality.csv), [16 encode fixture](2026-11-03-jpeg-chroma/fixtures/runs.json), [environment fixture](2026-11-03-jpeg-chroma/fixtures/environment.json).

JPEG/PNG tetap lokal pada `samples/results/jpeg-chroma-2026-11-03/` dan `samples/results/jpeg-chroma-fixtures-2026-11-03/`; file asli tidak berubah. Tes lokal:59 all-feature/all-target + doctest native,27 default termasuk doctest, delapan unit Python, Clippy default/native, fmt dan self-check benchmark lulus. Header420/progressive, decode RGB independen pada dimensi ganjil, ICC multi-marker dan kontrak target-byte tercakup. CI remote belum dijalankan.
