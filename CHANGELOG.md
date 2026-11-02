# Changelog

## 0.1.0 — unreleased

- Local baseline library: bounded static JPEG target-size compression and sample-exact PNG optimization.
- Optional CLI: explicit KB/KiB units, stdin/stdout, no-clobber file output, and cooperative cancellation.
- Opt-in structure-tensor prefilter and reproducible synthetic smoke benchmark.
- Tiled tensor prefilter for full-resolution JPEGs, cooperative tile checkpoints, and local sample ablation harness with explicit unreachable targets.
- Local samples excluded from Git and crate packages.
- Optimized progressive TurboJPEG encoding with 4:4:4 default, optional 4:2:0, bounded output, and ICC marker preservation; old image encoder retained for comparison.
- Opt-in rate-coupled tensor strength, counted reference probe, feasible-reference fallback, and Gaussian ablation control; desktop callers may explicitly raise filter limits to 50MP.
- No crates.io publication or GitHub repository creation yet.
