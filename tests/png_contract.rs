mod common;
use dt_image_compress::*;
use image::ImageEncoder;
use std::ops::ControlFlow;

fn run(input: &[u8], options: &CompressionOptions) -> Result<CompressionOutput, CompressionError> {
    compress(input, options, |_| ControlFlow::Continue(()))
}

fn lossless() -> CompressionOptions {
    CompressionOptions {
        mode: CompressionMode::Lossless,
        ..Default::default()
    }
}

#[test]
fn lossless_preserves_rgba_including_hidden_rgb() {
    let input = common::png();
    let out = run(&input, &lossless()).unwrap();
    assert_eq!(
        image::load_from_memory(&input).unwrap().to_rgba8(),
        image::load_from_memory(&out.bytes).unwrap().to_rgba8()
    );
    assert_eq!(out.report.output_format, OutputFormat::Png);
}

#[test]
fn lossless_preserves_sixteen_bit_samples() {
    let samples: Vec<u16> = (0..256).map(|i| i * 251).collect();
    let mut input = Vec::new();
    let native: Vec<u8> = samples.iter().flat_map(|n| n.to_ne_bytes()).collect();
    image::codecs::png::PngEncoder::new(&mut input)
        .write_image(&native, 16, 16, image::ExtendedColorType::L16)
        .unwrap();
    let out = run(&input, &lossless()).unwrap();
    assert_eq!(
        image::load_from_memory(&out.bytes)
            .unwrap()
            .to_luma16()
            .into_raw(),
        samples
    );
}

#[test]
fn removes_text_metadata_but_keeps_color_information() {
    let input = common::add_png_chunk(&common::png(), b"tEXt", b"Comment\0PRIVATE-SENTINEL");
    let input = common::add_png_chunk(&input, b"gAMA", &45455_u32.to_be_bytes());
    let out = run(&input, &lossless()).unwrap();
    assert!(!out.bytes.windows(16).any(|x| x == b"PRIVATE-SENTINEL"));
    assert!(out.bytes.windows(4).any(|x| x == b"gAMA"));
}

#[test]
fn png_cannot_silently_change_to_lossy_to_hit_a_target() {
    let options = CompressionOptions {
        target_bytes: Some(10),
        ..lossless()
    };
    assert!(matches!(
        run(&common::png(), &options),
        Err(CompressionError::TargetUnreachable { .. })
    ));
}
