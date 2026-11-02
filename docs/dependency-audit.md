# Dependency baseline

Pemeriksaan awal 2 November 2026; versi aktual dikunci di Cargo.lock.

| Dependency | Kegunaan | Lisensi sumber | Catatan |
| --- | --- | --- | --- |
| image 0.25.10 | Decode JPEG/PNG, encode JPEG, resize, normalisasi orientasi | MIT OR Apache-2.0 | Default formats/rayon dimatikan; MSRV upstream 1.88. |
| turbojpeg 1.5.1 | Decode piksel JPEG dan encode optimized progressive444/420 | Unlicense OR MIT untuk wrapper; native libjpeg-turbo memiliki pemberitahuan lisensi tersendiri | Hanya feature cmake; require-simd dimatikan. Decode menolak peringatan incomplete entropy; encoder memakai buffer terbatas dan mempertahankan ICC RGB. |
| mozjpeg-sys =2.2.3 | Opsional: MozJPEG, trellis dan adaptive DQT/Q_OPT | IJG AND Zlib AND BSD-3-Clause | API sys dibutuhkan karena wrapper mozjpeg0.10.13 tidak mengekspos Q_OPT. C-unwind + native -fexceptions dipakai untuk recovery error. Wajib shared vendor TurboJPEG agar simbol tidak bertabrakan; integrasi ini Linux saja. |
| jpegli-rust =0.1.1 | Opsional: native Google jpegli, AQ bawaan | Wrapper Apache-2.0; Google jpegli BSD-3-Clause; Highway Apache-2.0 OR BSD-3-Clause; notices header libjpeg-turbo juga berlaku | Rust>=1.90, C++17/CMake; native Linux/macOS saja, tanpa cross-compile. Source native dipin dan dibundel dalam crate, tanpa download build. |
| oxipng 10.2.1 | Optimasi PNG | MIT | Default features dimatikan; libdeflater tetap membawa native libdeflate, sehingga bukan seluruh dependency tree pure Rust. |
| thiserror 2 | Error terstruktur | MIT OR Apache-2.0 | Proc macro hanya untuk mengurangi boilerplate error. |
| clap 4.6 | Parsing CLI | MIT OR Apache-2.0 | Opsional melalui feature cli. |
| tempfile 3.27 | Temporary file dan publish tanpa clobber | MIT OR Apache-2.0 | Opsional melalui feature cli; bukan dependency library default. |
| ctrlc 3.5 | Cancel kooperatif melalui SIGINT/control event | MIT OR Apache-2.0 | Opsional melalui feature cli; memakai API OS melalui dependency transitif. |

Sumber: metadata registry crates.io, Cargo.toml release yang diunduh, serta [image](https://github.com/image-rs/image/tree/v0.25.10), [Oxipng](https://github.com/oxipng/oxipng), dan [turbojpeg API](https://docs.rs/turbojpeg/1.5.1/turbojpeg/struct.Decompressor.html). Tidak ada jpegli/zenjpeg, imagequant, FFmpeg, atau decoder HEIC dalam default build. `jpeg-decoder` 0.3.2 sempat diuji sebagai alternatif, tetapi juga menerima entropy terpotong dengan EOI ditambahkan; dependency percobaan tersebut sudah dilepas.

Sebelum publikasi: jalankan audit lisensi/security untuk dependency tree target distribusi, periksa availability nama crates.io, uji MSRV di CI, dan periksa ulang native build di Android ARM64/Linux. Tidak ada klaim bahwa semua target telah diuji.
## JPEG encoding follow-up

The existing `turbojpeg` 1.5.1 dependency now also encodes JPEG. Default: optimized Huffman, progressive, 4:4:4. Optional 4:2:0 is explicit. No new native library is added. `compress_to_slice` uses a bounded Rust-owned output allocation; ICC RGB is preserved using sequenced APP2 markers. Regression tests cover progressive header, selected chroma, and multi-marker ICC round-trip. The original `image` encoder remains a selectable baseline. [Upstream parameter documentation](https://github.com/libjpeg-turbo/libjpeg-turbo/blob/main/src/turbojpeg.h) explains optimize/progressive controls; the pinned Rust API was also inspected locally.

## Optional native JPEG integration

Published source archives and actual API/build files were inspected, then exercised on Linux x86_64/Rust1.93. [mozjpeg-sys2.2.3](https://docs.rs/crate/mozjpeg-sys/2.2.3/source/src/lib.rs) exposes `JBOOLEAN_TRELLIS_Q_OPT`; [MozJPEG semantics](https://github.com/mozilla/mozjpeg/blob/master/README-mozilla.txt) describe coefficient-derived quantization optimization. Tests prove fixed tables match between flat/textured inputs while Q_OPT changes textured-input tables, and outputs decode independently.

[jpegli-rust0.1.1](https://docs.rs/crate/jpegli-rust/0.1.1/source/) archive SHA-256: `b11de9cfcffd4c51baa5401fa76589ef93a74e311371de5c17629df7fcaaab6d`. Its native manifest pins Google jpegli `031a0077f5799a6041004267fc12b956c1f52a20` and Highway `271a9a0ed9de1232d9117f1572c3fe28f8542ec1`. The shim links prefixed `jpegli_*`; unprefixed libjpeg compatibility wrapper is disabled. `skcms` appears as configure-only source, not a linked shim dependency. Preserve the crate's bundled third-party notices and the actual native license files when packaging.

An isolated runtime/linker-map probe caught silent static-symbol substitution: MozJPEG calls resolved to TurboJPEG's `jpeg_CreateCompress`/`jpeg_set_defaults`. With vendor shared TurboJPEG they resolve to MozJPEG. All87 exported dynamic symbols of the tested shared library contain no `jpeg_*`/`jsimd_*`. `build.rs` enforces the tested Linux/shared/vendor configuration; runtime must load that build's `libturbojpeg.so.0`. This is a packaging requirement, not a claim of portable/mobile validation. Output caps are enforced; internal native allocations are not a process RSS hard cap.

