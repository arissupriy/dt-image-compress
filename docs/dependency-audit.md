# Dependency baseline

Pemeriksaan awal 2 November 2026; versi aktual dikunci di Cargo.lock.

| Dependency | Kegunaan | Lisensi sumber | Catatan |
| --- | --- | --- | --- |
| image 0.25.10 | Decode JPEG/PNG, encode JPEG, resize, normalisasi orientasi | MIT OR Apache-2.0 | Default formats/rayon dimatikan; MSRV upstream 1.88. |
| turbojpeg 1.5.1 | Decode piksel JPEG, termasuk penolakan peringatan incomplete entropy | Unlicense OR MIT untuk wrapper; native libjpeg-turbo memiliki pemberitahuan lisensi tersendiri | Hanya feature cmake; require-simd dimatikan. Kebutuhan CMake/native build diterima untuk menjaga kontrak input rusak. |
| oxipng 10.2.1 | Optimasi PNG | MIT | Default features dimatikan; libdeflater tetap membawa native libdeflate, sehingga bukan seluruh dependency tree pure Rust. |
| thiserror 2 | Error terstruktur | MIT OR Apache-2.0 | Proc macro hanya untuk mengurangi boilerplate error. |
| clap 4.6 | Parsing CLI | MIT OR Apache-2.0 | Opsional melalui feature cli. |
| tempfile 3.27 | Temporary file dan publish tanpa clobber | MIT OR Apache-2.0 | Opsional melalui feature cli; bukan dependency library default. |
| ctrlc 3.5 | Cancel kooperatif melalui SIGINT/control event | MIT OR Apache-2.0 | Opsional melalui feature cli; memakai API OS melalui dependency transitif. |

Sumber: metadata registry crates.io, Cargo.toml release yang diunduh, serta [image](https://github.com/image-rs/image/tree/v0.25.10), [Oxipng](https://github.com/oxipng/oxipng), dan [turbojpeg API](https://docs.rs/turbojpeg/1.5.1/turbojpeg/struct.Decompressor.html). Tidak ada jpegli/zenjpeg, imagequant, FFmpeg, atau decoder HEIC dalam default build. `jpeg-decoder` 0.3.2 sempat diuji sebagai alternatif, tetapi juga menerima entropy terpotong dengan EOI ditambahkan; dependency percobaan tersebut sudah dilepas.

Sebelum publikasi: jalankan audit lisensi/security untuk dependency tree target distribusi, periksa availability nama crates.io, uji MSRV di CI, dan periksa ulang native build di Android ARM64/Linux. Tidak ada klaim bahwa semua target telah diuji.
# JPEG encoding follow-up

The existing `turbojpeg` 1.5.1 dependency now also encodes JPEG. Default: optimized Huffman, progressive, 4:4:4. Optional 4:2:0 is explicit. No new native library is added. `compress_to_slice` uses a bounded Rust-owned output allocation; ICC RGB is preserved using sequenced APP2 markers. Regression tests cover progressive header, selected chroma, and multi-marker ICC round-trip. The original `image` encoder remains a selectable baseline. [Upstream parameter documentation](https://github.com/libjpeg-turbo/libjpeg-turbo/blob/main/src/turbojpeg.h) explains optimize/progressive controls; the pinned Rust API was also inspected locally.

