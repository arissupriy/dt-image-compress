mod common;
use dt_image_compress::*;
use std::ops::ControlFlow;

fn run(input: &[u8], options: &CompressionOptions) -> Result<CompressionOutput, CompressionError> {
    compress(input, options, |_| ControlFlow::Continue(()))
}

#[test]
fn meets_byte_target_and_keeps_best_measured_quality() {
    let input = common::jpeg(98);
    let options = CompressionOptions {
        target_bytes: Some(20_000),
        ..Default::default()
    };
    let out = run(&input, &options).unwrap();
    assert!(out.bytes.len() <= 20_000);
    assert_eq!(out.report.output_bytes, out.bytes.len() as u64);
    assert!(out.report.quality.unwrap() > 40);
    assert!(out.report.encode_attempts <= options.limits.max_encode_attempts);
    assert_eq!(out.report.target_search, Some(TargetSearch::LogSecant));
    assert_eq!(image::load_from_memory(&out.bytes).unwrap().width(), 512);
}

#[test]
fn log_secant_and_binary_agree_on_real_jpeg_quality_and_enforce_the_cap() {
    let input = common::jpeg(98);
    let mut options = CompressionOptions {
        target_bytes: Some(20_000),
        target_search: TargetSearch::Binary,
        ..Default::default()
    };
    let binary = run(&input, &options).unwrap();
    options.target_search = TargetSearch::LogSecant;
    let secant = run(&input, &options).unwrap();
    assert_eq!(secant.report.quality, binary.report.quality);
    assert_eq!(secant.bytes, binary.bytes);
    assert_eq!(secant.report.target_search, Some(TargetSearch::LogSecant));
    assert!(secant.bytes.len() <= 20_000);
    assert!(secant.report.encode_attempts <= options.limits.max_encode_attempts);
}

#[test]
#[cfg(any(feature = "mozjpeg", feature = "jpegli"))]
fn native_backends_compose_with_target_search_and_decode_independently() {
    let input = common::jpeg(98);
    let backends = [
        #[cfg(feature = "mozjpeg")]
        JpegBackend::Mozjpeg,
        #[cfg(feature = "jpegli")]
        JpegBackend::Jpegli,
        #[cfg(feature = "jpegli")]
        JpegBackend::Jpegli420,
    ];
    for backend in backends {
        let options = CompressionOptions {
            jpeg_backend: backend,
            target_search: TargetSearch::LogSecant,
            target_bytes: Some(20_000),
            #[cfg(feature = "mozjpeg")]
            adaptive_dqt: backend == JpegBackend::Mozjpeg,
            ..Default::default()
        };
        let out = run(&input, &options).unwrap();
        assert!(out.bytes.len() <= 20_000);
        assert_eq!(out.report.jpeg_backend, Some(backend));
        assert_eq!(out.report.final_dimensions, (512, 384));
        assert_eq!(image::load_from_memory(&out.bytes).unwrap().width(), 512);
        assert!(out.report.encode_attempts <= 32);
    }
}

#[test]
#[cfg(feature = "mozjpeg")]
fn adaptive_dqt_is_rejected_for_other_backends() {
    let options = CompressionOptions {
        adaptive_dqt: true,
        ..Default::default()
    };
    assert!(matches!(
        options.validate(),
        Err(CompressionError::InvalidOptions(_))
    ));
}

#[test]
fn unreachable_target_is_an_error_not_an_oversized_success() {
    let options = CompressionOptions {
        target_bytes: Some(10),
        ..Default::default()
    };
    assert!(matches!(
        run(&common::jpeg(95), &options),
        Err(CompressionError::TargetUnreachable {
            target_bytes: 10,
            ..
        })
    ));
}

#[test]
fn no_target_is_one_encode_at_quality_80_without_resize() {
    let out = run(&common::jpeg(98), &Default::default()).unwrap();
    assert_eq!(out.report.quality, Some(80));
    assert_eq!(out.report.encode_attempts, 1);
    assert_eq!(out.report.final_dimensions, (512, 384));
}

#[test]
fn normalizes_all_exif_orientations_and_removes_private_markers() {
    let original = common::jpeg(95);
    let source = image::load_from_memory(&original).unwrap().to_rgb8();
    for orientation in 1..=8 {
        let input = common::add_jpeg_exif(&original, orientation);
        let out = run(&input, &Default::default()).unwrap();
        let dims = if orientation >= 5 {
            (384, 512)
        } else {
            (512, 384)
        };
        assert_eq!(out.report.final_dimensions, dims);
        assert!(!out.bytes.windows(6).any(|x| x == b"Exif\0\0"));
        use image::imageops::{flip_horizontal, flip_vertical, rotate90, rotate180, rotate270};
        let expected = match orientation {
            1 => source.clone(),
            2 => flip_horizontal(&source),
            3 => rotate180(&source),
            4 => flip_vertical(&source),
            5 => flip_horizontal(&rotate90(&source)),
            6 => rotate90(&source),
            7 => flip_vertical(&rotate90(&source)),
            8 => rotate270(&source),
            _ => unreachable!(),
        };
        let actual = image::load_from_memory(&out.bytes).unwrap().to_rgb8();
        let absolute_error: u64 = actual
            .as_raw()
            .iter()
            .zip(expected.as_raw())
            .map(|(a, b)| u64::from(a.abs_diff(*b)))
            .sum();
        assert!(
            absolute_error as f64 / (actual.as_raw().len() as f64) < 12.0,
            "orientation={orientation}"
        );
    }
}

#[test]
fn transparency_requires_an_explicit_background() {
    let mut options = CompressionOptions {
        format: OutputFormat::Jpeg,
        ..Default::default()
    };
    assert!(matches!(
        run(&common::png(), &options),
        Err(CompressionError::UnsupportedOperation(_))
    ));
    options.alpha_background = Some([255, 255, 255]);
    let out = run(&common::png(), &options).unwrap();
    let img = image::load_from_memory(&out.bytes).unwrap().to_rgb8();
    assert!(img.get_pixel(5, 5).0.iter().all(|v| *v > 245));
}

#[test]
fn resize_requires_consent_and_obeys_the_floor() {
    let input = common::jpeg(98);
    let mut options = CompressionOptions {
        target_bytes: Some(2000),
        min_long_edge: 64,
        ..Default::default()
    };
    assert!(matches!(
        run(&input, &options),
        Err(CompressionError::TargetUnreachable { .. })
    ));
    options.allow_resize = true;
    let out = run(&input, &options).unwrap();
    assert!(out.bytes.len() <= 2000);
    assert!(out.report.resized);
    assert!(out.report.final_dimensions.0 >= 64);
    assert!(out.report.encode_attempts <= 32);
}

#[test]
fn cancel_during_encode_never_emits_complete() {
    let mut completed = false;
    let result = compress(&common::jpeg(95), &Default::default(), |p| {
        if p.stage == ProgressStage::Complete {
            completed = true;
        }
        if p.stage == ProgressStage::Encode {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    });
    assert!(matches!(result, Err(CompressionError::Cancelled)));
    assert!(!completed);
}

#[test]
fn truncated_entropy_is_rejected_even_with_an_added_end_marker() {
    let original = common::jpeg(95);
    for cut in [10, 100, original.len() / 4, original.len() / 2] {
        let truncated = &original[..original.len() - cut];
        assert!(
            matches!(
                run(truncated, &Default::default()),
                Err(CompressionError::InvalidInput(_))
            ),
            "cut={cut}"
        );
        let mut repaired_marker = truncated.to_vec();
        repaired_marker.extend_from_slice(&[0xff, 0xd9]);
        assert!(
            matches!(
                run(&repaired_marker, &Default::default()),
                Err(CompressionError::InvalidInput(_))
            ),
            "EOI-appended cut={cut}"
        );
    }
}

#[test]
fn cmyk_is_reported_as_unsupported_instead_of_corrupt() {
    let pixels = vec![100_u8; 16 * 16 * 4];
    let input = turbojpeg::compress(
        turbojpeg::Image {
            pixels: &pixels[..],
            width: 16,
            height: 16,
            pitch: 64,
            format: turbojpeg::PixelFormat::CMYK,
        },
        90,
        turbojpeg::Subsamp::None,
    )
    .unwrap();
    assert!(matches!(
        run(&input, &Default::default()),
        Err(CompressionError::UnsupportedOperation(_))
    ));
}

#[test]
fn default_jpeg_uses_progressive_coding_without_discarding_chroma_samples() {
    let output = run(&common::jpeg(95), &Default::default()).unwrap();
    let header = turbojpeg::read_header(&output.bytes).unwrap();
    assert!(header.is_progressive);
    assert_eq!(header.subsamp, turbojpeg::Subsamp::None);
}

#[cfg(feature = "experimental-prefilter")]
#[test]
fn rate_coupling_increases_filter_strength_and_counts_the_reference_encode() {
    let input = common::jpeg(95);
    let mut options = CompressionOptions {
        target_bytes: Some(1_000_000),
        prefilter: Some(experimental::PrefilterOptions {
            mode: experimental::PrefilterMode::RateCoupledTensor,
            ..Default::default()
        }),
        ..Default::default()
    };
    let mild = run(&input, &options).unwrap();
    options.target_bytes = Some(20_000);
    let strong = run(&input, &options).unwrap();
    assert!(strong.report.prefilter_threshold.unwrap() > mild.report.prefilter_threshold.unwrap());
    assert!(strong.bytes.len() <= 20_000);
    assert!(strong.report.encode_attempts <= options.limits.max_encode_attempts);
    assert!(mild.report.encode_attempts >= 2);
}

#[cfg(feature = "experimental-prefilter")]
#[test]
fn rate_coupling_rejects_missing_context_and_retains_a_feasible_reference_with_tiny_budget() {
    let input = common::jpeg(95);
    let mut options = CompressionOptions {
        prefilter: Some(experimental::PrefilterOptions {
            mode: experimental::PrefilterMode::RateCoupledTensor,
            ..Default::default()
        }),
        ..Default::default()
    };
    assert!(matches!(
        run(&input, &options),
        Err(CompressionError::InvalidOptions(_))
    ));
    let reference = run(&input, &Default::default()).unwrap();
    options.target_bytes = Some(reference.bytes.len() as u64);
    options.limits.max_encode_attempts = 2;
    let output = run(&input, &options).unwrap();
    assert!(output.bytes.len() <= reference.bytes.len());
    assert_eq!(output.report.encode_attempts, 2);
    assert_eq!(output.report.prefilter_threshold, None);
}

#[cfg(feature = "experimental-prefilter")]
#[test]
fn tiled_prefilter_handles_full_resolution_jpeg_within_the_memory_estimate() {
    let source = image::RgbImage::from_fn(1153, 897, |x, y| {
        image::Rgb([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8])
    });
    let mut input = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut input, 90)
        .encode_image(&source)
        .unwrap();
    let options = CompressionOptions {
        prefilter: Some(Default::default()),
        limits: ProcessingLimits {
            max_working_memory_bytes: 40 * 1024 * 1024,
            ..Default::default()
        },
        ..Default::default()
    };
    let output = run(&input, &options).unwrap();
    assert_eq!(output.report.final_dimensions, (1153, 897));
    assert!(!output.report.resized);
    assert_eq!(output.report.encode_attempts, 1);
    assert!(!output.report.warnings.is_empty());
    assert_eq!(
        image::load_from_memory(&output.bytes)
            .unwrap()
            .to_rgb8()
            .dimensions(),
        (1153, 897)
    );
}
