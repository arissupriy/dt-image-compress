# Execution ledger — plan: /home/arissupriy/Projects/DailyTools/PLANS.md

Scope: image compression only. Work locally while the user prepares the remote repositories.

- Ruling: use the independent `dt-image-compress/` source directory; there is no Git repository yet, so Git worktree/commit scripts do not apply. Do not create a parent Cargo workspace or a remote repository.
- Ruling: first deliver a JPEG/PNG library, CLI, reproducible baseline, and an opt-in tensor experiment. Additional image formats follow their own gates; Flutter/video are outside this implementation slice.
- Ruling: Rust MSRV 1.88, edition 2024; development toolchain 1.93.0. Pin reproducible dependency resolution in Cargo.lock. `image` 0.25.10 and `oxipng` 10.2.1 were checked against registry metadata and official documentation.
- Ruling: keep MIT OR Apache-2.0 as the proposed package licensing from PLANS.md; no publication is performed before the project owner finalizes distribution.
- Task T02–T05: RED tests written for input limits, target bytes, EXIF, transparency, and sample-exact PNG. Initial library has no implementation.

## Implemented slice

- T01 partial: API/limits, dependency decisions, research caveats, and benchmark protocol recorded. Full prior-art review, real evaluation corpus, final distribution audit, and registry/repository availability remain open.
- T02–T05 baseline: structured errors, validated JPEG/PNG input, estimated memory/decoder limits, bounded quality search, consent-based resize, EXIF 1–8, explicit alpha flatten, and sample-exact PNG (including 16-bit/hidden RGB).
- T06 baseline: optional `dt-img`, KB/KiB parser, bounded input, no-clobber temporary output publication, binary stdout, and cooperative SIGINT handling for blocked pipe I/O.
- T07 partial: correctly windowed tensor, opt-in tiled prefilter, numerical/edge/noise/seam tests, and reproducible synthetic/local-JPEG harnesses. Diagnostic full tensor maps remain capped at 1 MP. No rate-coupled threshold, DQT modification, log-secant, or custom SIMD yet.
- T14 partial: CI prepared for Rust 1.88/stable on Linux; remote CI, registry publication, and cross-platform distribution have not run.

## RED → GREEN evidence and review rulings

- Initial tests failed because public API was absent; baseline integration tests then passed.
- CLI tests failed against the empty binary, then passed after implementation.
- Tensor tests failed at unimplemented functions, then passed with Gaussian-windowed components.
- Independent review found blocked pipe cancellation, f32 threshold-square underflow, and silently accepted truncated JPEG entropy.
- Pipe cancellation regressions failed, then passed after cancellable I/O workers with 25 ms polling. Core cancellation remains cooperative between codec calls.
- Tiny normal/subnormal threshold regression turned uniform white black before the fix; f64 denominator calculation passed the same regression.
- Truncated JPEG regression failed with image/zune and with jpeg-decoder 0.3.2 even after an EOI marker was appended. Ruling: use turbojpeg 1.5.1 for pixel decode and reject its warnings; native build dependency is justified by integrity at the input boundary. The trial jpeg-decoder dependency was removed. Regression passed for cuts of 10/100 bytes and 25%/50%, with and without an added EOI marker.
- First complete native all-feature/all-target suite after fixes: 33 passed. Subsequent default-build check hit a disk-full linker failure, not a test assertion. Only this package's generated target/debug directory was removed; source, lockfile, and benchmark results were preserved. Remaining checks use debug symbols/incremental disabled for the constrained local disk.

Final command results and benchmark conclusions follow below.

## Final verification — 2 November 2026

- CMYK/YCCK valid input regression first failed as InvalidInput; native header guard now returns UnsupportedOperation, and README explicitly limits supported JPEG color spaces.
- Baseline and all-feature tests, rustfmt, and default/all-feature Clippy were run from the package with `--offline --locked`; local build environment set `CARGO_HOME=/tmp/dailytools-cargo-home`, `CARGO_INCREMENTAL=0`, and dev/test debug symbols to zero to fit available disk. These environment choices do not alter compression settings.
- Final `cargo test --offline --locked --all-features --all-targets -j 2`: **35 tests passed**. Default-feature test suite also passed, including **1 README/API doctest**. `cargo fmt --check` and default/all-feature `cargo clippy --all-targets -- -D warnings`: PASS.
- `cargo build --offline --locked --release --all-features -j 2`: PASS. Local executable is `target/release/dt-img`; 2,447,488 bytes, no remote publication.
- `cargo package --offline --locked --no-verify --allow-dirty`: package assembly PASS; this is not registry publication or a claim that packaging verification/remote CI ran.
- `python3 tools/benchmark.py --self-test`: PASS. Release benchmark: 45 rows, equal byte budgets, sample-exact transparent PNG, recorded Linux VmHWM. Raw results and source hashes preserved under `docs/benchmarks/2026-11-02-smoke/`.
- Decision: tensor prefilter remains opt-in. Synthetic size reduction 0–1.631% did not meet the proposed 5% gate; PSNR slightly regressed on several fixtures and elapsed time increased. See the benchmark report; no novelty or mobile-performance claim.
- Remaining: real licensed corpus, broader prior-art/ablation study, fuzzing, Android/ARM64, actual Rust 1.88 CI execution, additional formats, final dependency distribution audit, and any remote Git/registry work.

## JPEG-only follow-up — user samples, 2 November 2026

- User requested JPEG focus and a check of combined algorithms against PLANS.md. Three JPEGs in samples/ were inspected locally: 3.69 MP and two 8.29 MP inputs. They appear to be variants of the same noisy mountain illustration, not independent content classes. No original sample is uploaded or copied into reports.
- RED: >1MP JPEG prefilter integration failed with ResourceLimitExceeded("experimental scratch buffers"). GREEN: prefilter now processes 256x256 tiles with a 2px halo, preserving original resolution and using an estimated two RGB frames plus 8 MiB tile scratch. Full diagnostic tensor maps still have the 1MP cap. Seam/edge equality against the full reference and cancellation between tiles pass.
- Benchmark checks first failed when different targets were merged into one summary group and when PSNR silently accepted mismatched dimensions. Summary now groups by target/status; PSNR rejects mismatched dimensions. Python self-check also covers unreachable cases and oversized successes.
- Release local-JPEG benchmark: 36 sample/target/method combinations, one warm-up plus three repetitions each, 108 measured rows. 24 combinations succeeded, all within target; 12 returned TargetUnreachable. All 18 combinations allowing resize succeeded, using at most 19 encodes. Whole benchmark process peak VmHWM 200,012 KiB, including metric/reference buffers outside core timing. Timing was variable on the non-isolated host; no latency/speed-up claim.
- Raw CSV/summary/environment/RSS copied to docs/benchmarks/2026-11-02-jpeg-samples/. Generated JPEGs remain under ignored target/jpeg-samples-2026-11-02/. Cargo --list confirms no original samples/JPEGs in package; explicit package exclude protects samples even before Git initialization.
- Independent focused review found no important issues in tiling, memory estimates, cancellation, or metric bookkeeping. README/protocol were updated; comparisons between resized methods explicitly account for independently chosen dimensions.
- Verification: cargo test --offline --locked --all-features --all-targets -j 2: **39 passed**. Default cargo test (including README doctest), fmt, default/all-feature Clippy -D warnings, benchmark Python self-check, and release CLI/example builds passed.
- Decision: E1 remains opt-in. At target 1 MB the 1 MB source gained about 1.01 dB PSNR with E1; at 500 KB it lost 0.141 dB. Higher encoder quality or larger output dimensions alone do not prove better fidelity. No rate-coupled threshold, custom DQT, alternative encoder, OCR/SSIM, or Android validation was added. See docs/benchmarks/jpeg-samples-2026-11-02.md for the full plan gap matrix and results.

## Execute remaining JPEG hybrid work

- Initial local commits: DailyTools plan repository 33dbd29; independent crate 69c2787. Samples/results and build outputs are ignored. No remote push.
- Default encoder upgraded to libjpeg-turbo optimized Huffman/progressive 4:4:4; 4:2:0 is explicit, and the original image encoder remains selectable. A standalone system jpegtran experiment reduced the old 487,640-byte sample output to 428,539 bytes; ImageMagick decoded-pixel AE was zero. This specific result is entropy optimization, not a claim of universal savings.
- H2 RED: requested adaptive filtering had no effective threshold in the report. GREEN: a counted unfiltered q80 probe derives target pressure, bounded sqrt scaling sets threshold, and quality/scale searches keep that filter fixed. A measured feasible reference is retained if the remaining encode budget fails. Targets and at least two attempts are mandatory. Numerical pressure limits and a two-attempt fallback are tested.
- B2 Gaussian control added under experimental-prefilter. Tiled prefilter upper cap is 50MP when callers explicitly raise core budgets; core defaults remain20MP/256MiB. Full diagnostic maps stay capped at1MP.
- cargo test --offline --locked --all-features --all-targets -j2: **45 passed**; all-feature Clippy -D warnings and formatting passed. Fresh focused review found no blocking encoder/ICC/coupling/budget issues; one benchmark resize metadata label was corrected.
- Expanded local JPEG corpus run uses explicit desktop50MP/1GiB limits, seven backend/filter methods at200,000 bytes, one measured pass and no warmup. Quality/dimensions are the comparison target; these timings are not a latency benchmark. Results will be recorded separately after the run completes.

## Expanded corpus completed

- 13 original JPEGs, seven methods, 91/91 successful outputs <=200,000 bytes, at most32 encodes. All original SHA-256 values and recorded Rust source hashes match. Source sample images remain local/ignored; raw measurements and manifest are versioned under docs/benchmarks/2026-11-02-jpeg-suite/.
- Turbo444 vs image baseline:12 improved/1 regressed by RGB PSNR, mean+0.2761dB. E2-rate444 vs Turbo444 without filter:9 improved/4 regressed, mean−0.0049dB. Retain conservative optimized444 default; experimental filters and420 remain explicit choices. No novelty, perceptual superiority, or mobile-performance claim.
- JPEG outputs are in samples/results/jpeg-suite-2026-11-02/. A separate best-psnr-200KB/ directory contains13 copied outputs selected after measurement, with selection.csv. This is not production automatic selection. Input paths/hashes are mapped in environment.json.
- Release CLI target100KB smoke: sample-1mb.jpg ->98,345 bytes at2560x1440, q40, eight encodes, no resize, adaptive Turbo420. ImageMagick PSNR29.8982dB; JPEG remains lossy. Output stored directly under samples/results/.
- Peak whole benchmark RSS825,840KiB includes references/metrics. Desktop limits50MP/1GiB are explicit and core defaults20MP/256MiB unchanged. One measurement/no warmup plus concurrent build activity prevents timing conclusions.
- Independent review's resize metadata wording was corrected in the script and saved manifest. No metric data was altered. Corpus report: docs/benchmarks/jpeg-suite-2026-11-02.md.
- Final cargo test --all-features --all-targets: **47 passed** (including ramp/corner coverage). Default tests including doctest, default/all-feature Clippy -D warnings, fmt, benchmark self-check and release CLI build passed. Remaining research/platform gates are listed in the report and root plan.

## Bracketed log-secant

- RED: the new exponential-size test failed when log-secant delegated to binary search (same probe count). GREEN: endpoint measurements and interpolation in log bytes reduce probes on that curve while preserving the same highest feasible quality. Midpoint fallback prevents stalled interpolation; every candidate is measured and the encode budget remains bounded.
- Added explicit `TargetSearch` library/CLI selection and report field. Binary stays the default pending corpus comparison. This is a size-search method, composable with an encoder/filter, not another JPEG encoder.
- Two solver regressions pass, including plateaus, unreachable targets, and nonmonotonic measured sizes. All 11 JPEG contract tests pass, including byte-identical real-JPEG results from binary/log-secant at the same selected quality.

## Native MozJPEG, adaptive DQT, Google jpegli

- Published APIs/build sources were inspected before integration. MozJPEG uses pinned sys2.2.3 because safe wrapper0.10.13 has no Q_OPT control. Tests show actual input-dependent table changes with `JBOOLEAN_TRELLIS_Q_OPT`, independent output decode, progressive444,140KB RGB ICC across multiple APP2 markers, bounded output and native-error recovery through supported C-unwind/-fexceptions. `panic=abort` is rejected before the native call.
- Google jpegli is the real pinned native implementation via jpegli-rust0.1.1, with progressive444, native AQ enabled, checked pixel/output caps and ICC. Its prefixed API avoids libjpeg symbol collisions. Rust>=1.90 and native Linux/macOS only are upstream wrapper requirements; base package remains declarative1.88. CI separates base/MSRV from native stable-Linux jobs.
- A runtime/link-map probe exposed silent substitution when static TurboJPEG and MozJPEG export the same symbols. Shared vendor TurboJPEG hides those symbols; actual MozJPEG calls then resolve to MozJPEG. `build.rs` rejects static, external-source, and non-Linux MozJPEG coexistence. Standalone distribution must bundle the resulting shared TurboJPEG and configure its runtime path.
- Both encoders dispatch through the existing core, preserve target/decode/budget contracts, and compose with explicit log-secant/prefilter options. Adaptive DQT on another backend returns InvalidOptions. This does not chain two JPEG encoders or invent a new custom table policy.
- Focused review found a destination callback pointer narrowed to the first field; it now derives from the whole stable allocation with addr_of_mut!, confirmed resolved. No other actionable review findings. Native sanitizers and other platforms were not run.
- Complete integration suite after the pointer correction:58 tests passed. All-feature/all-target Clippy with -D warnings passed. Build guard self-check rejected static/non-vendor and accepted shared/vendor configuration. Corpus results follow separately.
