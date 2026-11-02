use crate::{CompressionError, CompressionOptions};
use image::RgbImage;

pub(super) fn encode(
    img: &RgbImage,
    quality: u8,
    icc: Option<&[u8]>,
    options: &CompressionOptions,
) -> Result<Vec<u8>, CompressionError> {
    let resource_error = || {
        CompressionError::ResourceLimitExceeded("jpegli pixel or output allocation limit".into())
    };
    if u64::from(img.width()) * u64::from(img.height()) > options.limits.max_pixels {
        return Err(resource_error());
    }
    let width = usize::try_from(img.width()).map_err(|_| resource_error())?;
    let height = usize::try_from(img.height()).map_err(|_| resource_error())?;
    let max_pixels = usize::try_from(options.limits.max_pixels).map_err(|_| resource_error())?;
    let max_output_bytes = usize::try_from(options.limits.max_working_memory_bytes / 4)
        .ok()
        .filter(|&limit| limit > 0 && limit <= isize::MAX as usize)
        .ok_or_else(resource_error)?;
    let stride = width.checked_mul(3).ok_or_else(resource_error)?;
    let encode_error = |error: jpegli_rust::Error| match error {
        jpegli_rust::Error::OutputLimit | jpegli_rust::Error::Allocation => {
            CompressionError::ResourceLimitExceeded(error.to_string())
        }
        _ => CompressionError::EncodeFailed(error.to_string()),
    };
    // Native Google jpegli uses its own quantization tables and enables AQ by default.
    let mut encoder = jpegli_rust::Encoder::new(
        width,
        height,
        jpegli_rust::Options {
            quality,
            scan_mode: jpegli_rust::ScanMode::JpegliDefault,
            subsampling: if options.jpeg_backend == crate::JpegBackend::Jpegli420 {
                jpegli_rust::Subsampling::S420
            } else {
                jpegli_rust::Subsampling::S444
            },
            max_pixels,
            max_output_bytes,
        },
    )
    .map_err(encode_error)?;
    if let Some(profile) = icc {
        encoder.write_icc_profile(profile).map_err(encode_error)?;
    }
    encoder
        .write_rows(img.as_raw(), height, stride)
        .map_err(encode_error)?;
    let jpeg = encoder.finish().map_err(encode_error)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(jpeg.len())
        .map_err(|_| resource_error())?;
    bytes.extend_from_slice(jpeg.as_ref());
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageDecoder;

    #[test]
    fn progressive_420_keeps_dimensions_and_rgb_channel_order() {
        let image = image::RgbImage::from_fn(33, 25, |x, y| {
            image::Rgb([(x * 6) as u8, (y * 8) as u8, 96])
        });
        let options = CompressionOptions {
            jpeg_backend: crate::JpegBackend::Jpegli420,
            ..Default::default()
        };
        let bytes = encode(&image, 95, None, &options).unwrap();
        let header = turbojpeg::read_header(&bytes).unwrap();
        assert!(header.is_progressive);
        assert_eq!(header.subsamp, turbojpeg::Subsamp::Sub2x2);
        let decoded = image::load_from_memory(&bytes).unwrap().into_rgb8();
        assert_eq!(decoded.dimensions(), image.dimensions());
        let error = image
            .as_raw()
            .iter()
            .zip(decoded.as_raw())
            .map(|(a, b)| u64::from(a.abs_diff(*b)))
            .sum::<u64>() as f64
            / image.as_raw().len() as f64;
        assert!(error < 6.0, "RGB mean absolute error: {error}");
    }

    #[test]
    fn progressive_444_preserves_rgb_pixels_and_dimensions() {
        let image = image::RgbImage::from_fn(33, 25, |x, y| {
            image::Rgb([(x * 6) as u8, (y * 8) as u8, 96])
        });
        let bytes = encode(&image, 95, None, &CompressionOptions::default()).unwrap();
        let header = turbojpeg::read_header(&bytes).unwrap();
        assert!(header.is_progressive);
        assert_eq!(header.subsamp, turbojpeg::Subsamp::None);
        let decoded = image::load_from_memory_with_format(&bytes, image::ImageFormat::Jpeg)
            .unwrap()
            .into_rgb8();
        assert_eq!(decoded.dimensions(), (33, 25));
        let mean_error = image
            .as_raw()
            .iter()
            .zip(decoded.as_raw())
            .map(|(a, b)| u64::from(a.abs_diff(*b)))
            .sum::<u64>() as f64
            / image.as_raw().len() as f64;
        assert!(mean_error < 6.0, "RGB mean absolute error: {mean_error}");
    }

    #[test]
    fn retains_multimarker_icc() {
        let image = image::RgbImage::from_pixel(17, 13, image::Rgb([20, 100, 200]));
        let mut profile = vec![42_u8; 140_000];
        profile[16..20].copy_from_slice(b"RGB ");
        for backend in [crate::JpegBackend::Jpegli, crate::JpegBackend::Jpegli420] {
            let options = CompressionOptions {
                jpeg_backend: backend,
                ..Default::default()
            };
            let bytes = encode(&image, 85, Some(&profile), &options).unwrap();
            let mut decoder =
                image::codecs::jpeg::JpegDecoder::new(std::io::Cursor::new(bytes)).unwrap();
            assert_eq!(decoder.icc_profile().unwrap().unwrap(), profile);
        }
    }

    #[test]
    fn pixel_and_output_limits_return_resource_errors() {
        let image = image::RgbImage::from_pixel(8, 8, image::Rgb([20, 100, 200]));
        let mut options = CompressionOptions::default();
        options.limits.max_pixels = 63;
        assert!(matches!(
            encode(&image, 85, None, &options),
            Err(CompressionError::ResourceLimitExceeded(_))
        ));
        options.limits.max_pixels = 64;
        options.limits.max_working_memory_bytes = 128;
        assert!(matches!(
            encode(&image, 85, None, &options),
            Err(CompressionError::ResourceLimitExceeded(_))
        ));
    }
}
