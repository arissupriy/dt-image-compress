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

Unit test menguji field konstan, noise, edge, parameter invalid, gambar kecil, serta equality tile terhadap referensi penuh di semua piksel termasuk seam. Filter kompresi memakai tile 256×256 dan halo 2 piksel. API diagnostik tensor penuh tetap dibatasi 1 megapiksel. Cancel diperiksa antar-tile. Optimasi SIMD khusus pada filter belum diimplementasikan.

H2 kini diimplementasikan opt-in: `rho=max(1,B_q80_unfiltered/B_target)`, `tau=min(max(tau_base,.25),tau_base*sqrt(rho))`. Referensi q80 berasal dari backend yang dipilih, bukan ukuran file sumber yang bisa dipengaruhi metadata/kualitas encode sebelumnya. Probe memakai satu jatah encode; target wajib dan budget minimum dua. Threshold tetap untuk seluruh pencarian quality/skala agar solver tidak menghadapi konfigurasi yang berubah di tengah bracket. Kandidat referensi yang sudah feasible dipertahankan sebagai fallback. Ini menguji hipotesis coupling, bukan pembuktian fungsi tau optimal.

Backend pembanding yang terintegrasi: image baseline, libjpeg-turbo progressive/Huffman teroptimasi 4:4:4, dan 4:2:0 eksplisit; ditambah feature opsional native MozJPEG dan Google jpegli. B2 memakai Gaussian blur sigma .8, E1 tensor tetap, E2 tensor terkopel. Angka quality tidak disamakan antarbackend untuk menyimpulkan fidelity; pembandingan utama memakai batas bytes sama dan dimensi dilaporkan.

Adaptive DQT kini memakai implementasi nyata MozJPEG `JBOOLEAN_TRELLIS_Q_OPT`, yang menurunkan tabel dari koefisien aktual. Unit test membedakan tabel input flat/tekstur dengan kontrol fixed-DQT. Ini bukan implementasi baru StealthMRC atau kebijakan tabel kustom dari paper. Google jpegli membawa native AQ yang aktif secara default, tanpa toggle dari wrapper; AQ tersebut berbeda dari optimasi tabel MozJPEG.

Log-secant sudah tersedia sebagai metode pencarian ukuran: endpoint40/95, interpolasi pada log bytes, bracket integer yang menyusut, fallback midpoint, dan hard cap pada kandidat terukur. Binary tetap default. Tes kurva eksponensial menguji pengurangan probe dan tes JPEG nyata memeriksa kesamaan kandidat; kedua metode mengasumsikan ukuran kira-kira monoton terhadap quality, tanpa klaim optimum global pada encoder nonmonoton. Semua metode ini dapat digabungkan dengan satu backend encoder dan prefilter opsional. Batas build/linkage backend native tercatat di README/audit dependency.

Prior art yang perlu diperdalam mencakup perceptual preprocessing, ROI coding, MRC, JPEG psychovisual encoders, dan target-byte optimization. Dua referensi dari diskusi Gemini: [CVPR 2026 — Ma dkk.](https://openaccess.thecvf.com/content/CVPR2026/html/Ma_Learned_Image_Compression_via_Sparse_Attention_and_Adaptive_Frequency_CVPR_2026_paper.html) dan [AAAI 2026 — SmartSplat](https://ojs.aaai.org/index.php/AAAI/article/view/37558). Verifikasi bibliografi tidak berarti hasil paper telah direproduksi atau metode ini mempunyai novelty yang terbukti.

[Hasil smoke benchmark pertama](benchmarks/smoke-2026-11-02.md): reduksi ukuran 0–1,631% dengan PSNR sedikit menurun pada beberapa fixture dan latency meningkat. Konfigurasi saat ini tetap experimental dan tidak menjadi default.

[Pengujian tiga JPEG awal](benchmarks/jpeg-samples-2026-11-02.md) memakai resolusi asli dan resize yang dilaporkan eksplisit, pada target 200 KB/500 KB/1 MB. H2 belum tersedia pada snapshot awal itu.

[Corpus13 JPEG dan tujuh metode](benchmarks/jpeg-suite-2026-11-02.md) kini menguji B2/E1/E2/E3 pada200KB:91/91 sukses. Turbo444 meningkatkan PSNR pada12/13 input vs image baseline; coupling444 memberi rata-rata−0,0049dB dibanding Turbo444 tanpa filter. Ini hasil negatif/terbatas yang tetap disimpan. Default encoder ditingkatkan, filter tetap opt-in, dan420 merupakan pilihan eksplisit. Tidak ada pembuktian bahwa coupling atau satu konfigurasi selalu unggul.
