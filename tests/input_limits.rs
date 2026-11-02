mod common;
use dt_image_compress::*;
use std::ops::ControlFlow;

fn run(input: &[u8], options: &CompressionOptions) -> Result<CompressionOutput, CompressionError> {
    compress(input, options, |_| ControlFlow::Continue(()))
}

#[test]
fn validates_options_before_decoding() {
    let options = CompressionOptions {
        target_bytes: Some(0),
        ..Default::default()
    };
    assert!(matches!(
        run(&[], &options),
        Err(CompressionError::InvalidOptions(_))
    ));
}

#[test]
fn rejects_empty_truncated_and_unsupported_input() {
    for data in [&[][..], b"not an image", b"\x89PNG\r\n\x1a\n"] {
        assert!(run(data, &Default::default()).is_err());
    }
    assert!(matches!(
        run(b"GIF89a", &Default::default()),
        Err(CompressionError::UnsupportedFormat)
    ));
}

#[test]
fn enforces_input_pixel_and_memory_limits() {
    let input = common::jpeg(90);
    let mut options = CompressionOptions::default();
    options.limits.max_input_bytes = 10;
    assert!(matches!(
        run(&input, &options),
        Err(CompressionError::ResourceLimitExceeded(_))
    ));
    options.limits = ProcessingLimits {
        max_pixels: 100,
        ..Default::default()
    };
    assert!(matches!(
        run(&input, &options),
        Err(CompressionError::ResourceLimitExceeded(_))
    ));
    options.limits = ProcessingLimits {
        max_working_memory_bytes: 1024,
        ..Default::default()
    };
    assert!(matches!(
        run(&input, &options),
        Err(CompressionError::ResourceLimitExceeded(_))
    ));
}

#[test]
fn supports_cooperative_cancel() {
    let err = compress(&common::jpeg(90), &Default::default(), |_| {
        ControlFlow::Break(())
    })
    .unwrap_err();
    assert!(matches!(err, CompressionError::Cancelled));
}

#[test]
fn rejects_animated_png_instead_of_using_first_frame() {
    let mut actl = 2_u32.to_be_bytes().to_vec();
    actl.extend_from_slice(&0_u32.to_be_bytes());
    let input = common::add_png_chunk(&common::png(), b"acTL", &actl);
    assert!(matches!(
        run(&input, &Default::default()),
        Err(CompressionError::UnsupportedOperation(_))
    ));
}
