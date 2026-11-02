use crate::{CompressionError, CompressionOptions, OutputFormat, error::decode_error};
use image::{ImageDecoder, ImageFormat, ImageReader};
use std::io::Cursor;

#[derive(Debug, Clone)]
pub struct InputInfo {
    pub format: OutputFormat,
    pub dimensions: (u32, u32),
    pub color_type: image::ColorType,
}

pub(crate) fn decode_jpeg(
    input: &[u8],
    options: &CompressionOptions,
) -> Result<image::DynamicImage, CompressionError> {
    // Treat libjpeg-turbo warnings (including incomplete entropy) as errors.
    let mut decoder = turbojpeg::Decompressor::new()
        .map_err(|e| CompressionError::InvalidInput(e.to_string()))?;
    decoder
        .set_scan_limit(100)
        .map_err(|e| CompressionError::InvalidInput(e.to_string()))?;
    let header = decoder
        .read_header(input)
        .map_err(|e| CompressionError::InvalidInput(e.to_string()))?;
    if matches!(
        header.colorspace,
        turbojpeg::Colorspace::CMYK | turbojpeg::Colorspace::YCCK
    ) {
        return Err(CompressionError::UnsupportedOperation(
            "CMYK/YCCK JPEG requires a color-managed conversion that is not supported yet".into(),
        ));
    }
    let length = header
        .width
        .checked_mul(header.height)
        .and_then(|n| n.checked_mul(3))
        .filter(|n| *n as u64 <= options.limits.max_working_memory_bytes / 4)
        .ok_or_else(|| CompressionError::ResourceLimitExceeded("JPEG RGB buffer".into()))?;
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(length)
        .map_err(|_| CompressionError::ResourceLimitExceeded("JPEG allocation".into()))?;
    pixels.resize(length, 0);
    decoder
        .decompress(
            input,
            turbojpeg::Image {
                pixels: &mut pixels[..],
                width: header.width,
                height: header.height,
                pitch: header.width * 3,
                format: turbojpeg::PixelFormat::RGB,
            },
        )
        .map_err(|e| CompressionError::InvalidInput(e.to_string()))?;
    image::RgbImage::from_raw(header.width as u32, header.height as u32, pixels)
        .map(image::DynamicImage::ImageRgb8)
        .ok_or_else(|| CompressionError::InvalidInput("JPEG pixel buffer layout".into()))
}

pub(crate) fn decoder<'a>(
    input: &'a [u8],
    options: &CompressionOptions,
) -> Result<impl ImageDecoder + 'a, CompressionError> {
    if input.len() as u64 > options.limits.max_input_bytes {
        return Err(CompressionError::ResourceLimitExceeded(
            "input bytes".into(),
        ));
    }
    let format = image::guess_format(input).map_err(decode_error)?;
    if !matches!(format, ImageFormat::Jpeg | ImageFormat::Png) {
        return Err(CompressionError::UnsupportedFormat);
    }
    if format == ImageFormat::Png {
        png_chunks(input, |kind, _| {
            if kind == b"acTL" {
                return Err(CompressionError::UnsupportedOperation(
                    "animated PNG".into(),
                ));
            }
            Ok(())
        })?;
    }
    let mut reader = ImageReader::with_format(Cursor::new(input), format);
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(options.limits.max_working_memory_bytes);
    let dimension_limit = options.limits.max_pixels.min(u64::from(u32::MAX)) as u32;
    limits.max_image_width = Some(dimension_limit);
    limits.max_image_height = Some(dimension_limit);
    reader.limits(limits);
    let decoder = reader.into_decoder().map_err(decode_error)?;
    let (w, h) = decoder.dimensions();
    let pixels = u64::from(w)
        .checked_mul(u64::from(h))
        .ok_or_else(|| CompressionError::ResourceLimitExceeded("dimension overflow".into()))?;
    if pixels == 0 || pixels > options.limits.max_pixels {
        return Err(CompressionError::ResourceLimitExceeded(
            "pixel count".into(),
        ));
    }
    // shortcut: codec scratch allocation is estimated; use process isolation for a strict RSS cap.
    let estimate = decoder
        .total_bytes()
        .checked_mul(5)
        .and_then(|n| n.checked_add((input.len() as u64).saturating_mul(3)))
        .and_then(|n| n.checked_add(4 * 1024 * 1024));
    if estimate.is_none_or(|n| n > options.limits.max_working_memory_bytes) {
        return Err(CompressionError::ResourceLimitExceeded(
            "estimated working memory".into(),
        ));
    }
    Ok(decoder)
}

pub fn inspect_input(
    input: &[u8],
    options: &CompressionOptions,
) -> Result<InputInfo, CompressionError> {
    options.validate()?;
    let decoder = decoder(input, options)?;
    Ok(InputInfo {
        format: if input.starts_with(b"\x89PNG") {
            OutputFormat::Png
        } else {
            OutputFormat::Jpeg
        },
        dimensions: decoder.dimensions(),
        color_type: decoder.color_type(),
    })
}

pub(crate) fn png_chunks(
    input: &[u8],
    mut visit: impl FnMut(&[u8; 4], &[u8]) -> Result<(), CompressionError>,
) -> Result<(), CompressionError> {
    let mut pos: usize = 8;
    while pos < input.len() {
        let header = input
            .get(pos..pos.saturating_add(8))
            .ok_or_else(|| CompressionError::InvalidInput("truncated PNG chunk".into()))?;
        let size = u32::from_be_bytes(header[..4].try_into().unwrap()) as usize;
        let end = pos
            .checked_add(12)
            .and_then(|n| n.checked_add(size))
            .filter(|n| *n <= input.len())
            .ok_or_else(|| CompressionError::InvalidInput("truncated PNG data".into()))?;
        visit(header[4..8].try_into().unwrap(), &input[pos + 8..end - 4])?;
        pos = end;
        if &header[4..8] == b"IEND" {
            if pos != input.len() {
                return Err(CompressionError::InvalidInput("trailing PNG data".into()));
            }
            return Ok(());
        }
    }
    Err(CompressionError::InvalidInput("missing PNG IEND".into()))
}
