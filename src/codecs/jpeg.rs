use crate::{CompressionError, CompressionOptions, JpegBackend};
use image::{ImageEncoder, RgbImage};
use std::io::{self, Write};

struct BoundedBuffer {
    bytes: Vec<u8>,
    limit: u64,
}
impl Write for BoundedBuffer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if (self.bytes.len() as u64).saturating_add(buf.len() as u64) > self.limit {
            return Err(io::Error::other("encoded buffer exceeds memory budget"));
        }
        self.bytes.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(crate) fn encode(
    img: &RgbImage,
    quality: u8,
    icc: Option<&[u8]>,
    options: &CompressionOptions,
) -> Result<Vec<u8>, CompressionError> {
    if options.jpeg_backend == JpegBackend::ImageBaseline {
        return encode_baseline(img, quality, icc, options);
    }
    let encode_error = |e: turbojpeg::Error| CompressionError::EncodeFailed(e.to_string());
    let mut compressor = turbojpeg::Compressor::new().map_err(encode_error)?;
    compressor
        .set_quality(i32::from(quality))
        .map_err(encode_error)?;
    compressor
        .set_subsamp(if options.jpeg_backend == JpegBackend::Turbo420 {
            turbojpeg::Subsamp::Sub2x2
        } else {
            turbojpeg::Subsamp::None
        })
        .map_err(encode_error)?;
    compressor.set_optimize(true).map_err(encode_error)?;
    compressor.set_progressive(true).map_err(encode_error)?;
    let limit = options.limits.max_working_memory_bytes / 4;
    let capacity = compressor
        .buf_len(img.width() as usize, img.height() as usize)
        .map_err(encode_error)?
        .min(usize::try_from(limit).unwrap_or(usize::MAX));
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(capacity)
        .map_err(|_| CompressionError::ResourceLimitExceeded("JPEG output allocation".into()))?;
    bytes.resize(capacity, 0);
    let len = compressor
        .compress_to_slice(
            turbojpeg::Image {
                pixels: img.as_raw(),
                width: img.width() as usize,
                height: img.height() as usize,
                pitch: img.width() as usize * 3,
                format: turbojpeg::PixelFormat::RGB,
            },
            &mut bytes,
        )
        .map_err(encode_error)?;
    bytes.truncate(len);
    if let Some(profile) = icc {
        // APP2 payload: 12-byte signature, sequence, count, then at most 65,519 profile bytes.
        const CHUNK: usize = 65_519;
        let count = profile.len().div_ceil(CHUNK);
        if count > 255 {
            return Err(CompressionError::UnsupportedOperation(
                "ICC profile exceeds JPEG marker capacity".into(),
            ));
        }
        let mut out = BoundedBuffer {
            bytes: Vec::new(),
            limit,
        };
        out.write_all(&bytes[..2])
            .map_err(|e| CompressionError::EncodeFailed(e.to_string()))?;
        for (index, chunk) in profile.chunks(CHUNK).enumerate() {
            let length = (chunk.len() + 16) as u16;
            let mut marker = vec![0xff, 0xe2];
            marker.extend_from_slice(&length.to_be_bytes());
            marker.extend_from_slice(b"ICC_PROFILE\0");
            marker.extend_from_slice(&[(index + 1) as u8, count as u8]);
            marker.extend_from_slice(chunk);
            out.write_all(&marker)
                .map_err(|e| CompressionError::EncodeFailed(e.to_string()))?;
        }
        out.write_all(&bytes[2..])
            .map_err(|e| CompressionError::EncodeFailed(e.to_string()))?;
        bytes = out.bytes;
    }
    Ok(bytes)
}

fn encode_baseline(
    img: &RgbImage,
    quality: u8,
    icc: Option<&[u8]>,
    options: &CompressionOptions,
) -> Result<Vec<u8>, CompressionError> {
    let mut out = BoundedBuffer {
        bytes: Vec::new(),
        limit: options.limits.max_working_memory_bytes / 4,
    };
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality);
    if let Some(icc) = icc {
        encoder
            .set_icc_profile(icc.to_vec())
            .map_err(|e| CompressionError::EncodeFailed(e.to_string()))?;
    }
    encoder
        .encode_image(img)
        .map_err(|e| CompressionError::EncodeFailed(e.to_string()))?;
    Ok(out.bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageDecoder;

    #[test]
    fn optimized_jpeg_retains_multimarker_icc_and_uses_selected_chroma() {
        let image = RgbImage::from_pixel(33, 25, image::Rgb([20, 100, 200]));
        let mut profile = vec![42_u8; 140_000];
        profile[16..20].copy_from_slice(b"RGB ");
        for (backend, subsamp) in [
            (JpegBackend::Turbo444, turbojpeg::Subsamp::None),
            (JpegBackend::Turbo420, turbojpeg::Subsamp::Sub2x2),
        ] {
            let options = CompressionOptions {
                jpeg_backend: backend,
                ..Default::default()
            };
            let bytes = encode(&image, 80, Some(&profile), &options).unwrap();
            let header = turbojpeg::read_header(&bytes).unwrap();
            assert!(header.is_progressive);
            assert_eq!(header.subsamp, subsamp);
            let mut decoder =
                image::codecs::jpeg::JpegDecoder::new(std::io::Cursor::new(bytes)).unwrap();
            assert_eq!(decoder.icc_profile().unwrap().unwrap(), profile);
        }
    }
}
