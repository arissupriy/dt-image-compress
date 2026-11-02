# Riset yang sedang diuji

Baseline produk tidak memerlukan algoritma baru: codec standar, batas byte yang benar, PNG sample-exact, dan penyimpanan aman sudah berguna.

Hipotesis awal: prefilter berbobot struktur dapat mengurangi noise latar sehingga encoder JPEG dapat mempertahankan lebih banyak detail pada anggaran bytes sama. Belum ada klaim novelty atau jaminan penghematan.

Eksperimen saat ini:

1. Luma proxy nonlinear dari kanal sRGB 8-bit.
2. Gradien Scharr 3×3 dengan normalisasi 1/32 dan border clamp.
3. Outer product gradien, lalu Gaussian window `[1,2,1] × [1,2,1] / 16` pada J11/J12/J22.
4. Energi `J11 + J22`, selisih eigenvalue melalui hypot, koherensi terbatas [0,1], orientasi memakai atan2.
5. Bobot `E*(1+k*C)/(E*(1+k*C)+threshold²)` untuk blend citra asli dan Gaussian blur.

Smoothing komponen tensor diperlukan agar tensor tidak selalu rank satu. Rumus ini mengukur struktur lokal, bukan identitas semantik teks/wajah. Tidak menggunakan identitas DCT yang keliru untuk bobot yang berubah antar-piksel.

Unit test menguji field konstan, noise, edge, parameter invalid, gambar kecil, serta equality tile terhadap referensi penuh di semua piksel termasuk seam. Filter kompresi memakai tile 256×256 dan halo 2 piksel untuk memproses JPEG pengguna di atas 1 megapiksel dengan scratch terbatas. API diagnostik tensor penuh tetap dibatasi 1 megapiksel. Cancel diperiksa antar-tile. Benchmark membandingkan JPEG tanpa/dengan prefilter pada batas bytes identik. Filter belum melakukan rate-coupled threshold, adaptive DQT, log-secant, atau optimasi SIMD khusus.

Prior art yang perlu diperdalam mencakup perceptual preprocessing, ROI coding, MRC, JPEG psychovisual encoders, dan target-byte optimization. Dua referensi dari diskusi Gemini: [CVPR 2026 — Ma dkk.](https://openaccess.thecvf.com/content/CVPR2026/html/Ma_Learned_Image_Compression_via_Sparse_Attention_and_Adaptive_Frequency_CVPR_2026_paper.html) dan [AAAI 2026 — SmartSplat](https://ojs.aaai.org/index.php/AAAI/article/view/37558). Verifikasi bibliografi tidak berarti hasil paper telah direproduksi atau metode ini mempunyai novelty yang terbukti.

[Hasil smoke benchmark pertama](benchmarks/smoke-2026-11-02.md): reduksi ukuran 0–1,631% dengan PSNR sedikit menurun pada beberapa fixture dan latency meningkat. Konfigurasi saat ini tetap experimental dan tidak menjadi default.

[Pengujian tiga JPEG pengguna](benchmarks/jpeg-samples-2026-11-02.md) memakai resolusi asli dan resize yang dilaporkan eksplisit, pada target 200 KB/500 KB/1 MB. Hasil tensor bercampur: ada perbaikan PSNR sekitar 1 dB pada satu kasus dan penurunan pada kasus lain. Ketiga input tampak merupakan variasi ilustrasi yang sama; ini belum membuktikan manfaat pada foto/dokumen beragam. H2/rate coupling belum diimplementasikan.
