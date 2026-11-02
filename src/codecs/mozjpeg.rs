use crate::{CompressionError, CompressionOptions};
use image::RgbImage;
use mozjpeg_sys as ffi;
use std::io::{self, Write};
use std::panic::{catch_unwind, resume_unwind};

struct BoundedBuffer {
    bytes: Vec<u8>,
    limit: u64,
}

impl Write for BoundedBuffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if (self.bytes.len() as u64).saturating_add(bytes.len() as u64) > self.limit {
            return Err(io::Error::other("encoded buffer exceeds memory budget"));
        }
        self.bytes
            .try_reserve_exact(bytes.len())
            .map_err(io::Error::other)?;
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

// The C destination pointer addresses the first field; the box keeps it stable.
#[repr(C)]
struct Destination {
    interface: ffi::jpeg_destination_mgr,
    output: BoundedBuffer,
    buffer: [u8; 16 * 1024],
}

impl Destination {
    fn reset(&mut self) {
        self.interface.next_output_byte = self.buffer.as_mut_ptr();
        self.interface.free_in_buffer = self.buffer.len();
    }
}

fn write_error(error: io::Error) -> ! {
    resume_unwind(Box::new(CompressionError::ResourceLimitExceeded(
        error.to_string(),
    )))
}

unsafe extern "C-unwind" fn init_destination(info: &mut ffi::jpeg_compress_struct) {
    unsafe { (&mut *info.dest.cast::<Destination>()).reset() };
}

unsafe extern "C-unwind" fn empty_output_buffer(
    info: &mut ffi::jpeg_compress_struct,
) -> ffi::boolean {
    let destination = unsafe { &mut *info.dest.cast::<Destination>() };
    if let Err(error) = destination.output.write_all(&destination.buffer) {
        write_error(error);
    }
    destination.reset();
    1
}

unsafe extern "C-unwind" fn term_destination(info: &mut ffi::jpeg_compress_struct) {
    let destination = unsafe { &mut *info.dest.cast::<Destination>() };
    let used = destination.buffer.len() - destination.interface.free_in_buffer;
    if let Err(error) = destination.output.write_all(&destination.buffer[..used]) {
        write_error(error);
    }
}

unsafe extern "C-unwind" fn native_error(info: &mut ffi::jpeg_common_struct) {
    let code = unsafe { (*info.err).msg_code };
    resume_unwind(Box::new(CompressionError::EncodeFailed(format!(
        "MozJPEG native error {code}"
    ))));
}

struct Compressor {
    info: Box<ffi::jpeg_compress_struct>,
    _error: Box<ffi::jpeg_error_mgr>,
    destination: Box<Destination>,
}

impl Drop for Compressor {
    fn drop(&mut self) {
        // The error manager and destination outlive destruction of the native compressor.
        unsafe { ffi::jpeg_destroy_compress(&mut self.info) };
    }
}

pub(crate) fn encode(
    img: &RgbImage,
    quality: u8,
    icc: Option<&[u8]>,
    options: &CompressionOptions,
    adaptive_dqt: bool,
) -> Result<Vec<u8>, CompressionError> {
    if cfg!(panic = "abort") {
        return Err(CompressionError::UnsupportedOperation(
            "MozJPEG needs panic=unwind to recover native encoding errors".into(),
        ));
    }
    if img.width() == 0 || img.height() == 0 || !(1..=100).contains(&quality) {
        return Err(CompressionError::InvalidOptions(
            "MozJPEG needs nonempty RGB pixels and quality 1..=100".into(),
        ));
    }
    const ICC_CHUNK: usize = 65_519;
    if icc.is_some_and(|profile| profile.len().div_ceil(ICC_CHUNK) > 255) {
        return Err(CompressionError::UnsupportedOperation(
            "ICC profile exceeds JPEG marker capacity".into(),
        ));
    }
    catch_unwind(|| {
        // All-zero libjpeg records are the documented starting state before jpeg_CreateCompress.
        let mut error = Box::new(unsafe { std::mem::zeroed::<ffi::jpeg_error_mgr>() });
        unsafe { ffi::jpeg_std_error(&mut error) };
        error.error_exit = Some(native_error);
        let mut compressor = Compressor {
            info: Box::new(unsafe { std::mem::zeroed::<ffi::jpeg_compress_struct>() }),
            _error: error,
            destination: Box::new(Destination {
                interface: ffi::jpeg_destination_mgr {
                    next_output_byte: std::ptr::null_mut(),
                    free_in_buffer: 0,
                    init_destination: Some(init_destination),
                    empty_output_buffer: Some(empty_output_buffer),
                    term_destination: Some(term_destination),
                },
                output: BoundedBuffer {
                    bytes: Vec::new(),
                    limit: options.limits.max_working_memory_bytes / 4,
                },
                buffer: [0; 16 * 1024],
            }),
        };
        compressor.info.common.err = &mut *compressor._error;
        let info = &mut *compressor.info;
        unsafe {
            ffi::jpeg_create_compress(info);
            info.image_width = img.width();
            info.image_height = img.height();
            info.in_color_space = ffi::JCS_RGB;
            info.input_components = 3;
            ffi::jpeg_set_defaults(info);
            ffi::jpeg_set_quality(info, i32::from(quality), 1);
            ffi::jpeg_c_set_bool_param(info, ffi::JBOOLEAN_TRELLIS_Q_OPT, adaptive_dqt.into());
            ffi::jpeg_simple_progression(info);
            // Preserve full-resolution chroma, like the default Turbo444 backend.
            for index in 0..info.num_components as usize {
                let component = &mut *info.comp_info.add(index);
                component.h_samp_factor = 1;
                component.v_samp_factor = 1;
            }
            let destination: *mut Destination = &mut *compressor.destination;
            info.dest = std::ptr::addr_of_mut!((*destination).interface);
            ffi::jpeg_start_compress(info, 1);
            if let Some(profile) = icc {
                let count = profile.len().div_ceil(ICC_CHUNK) as u8;
                for (index, chunk) in profile.chunks(ICC_CHUNK).enumerate() {
                    let mut marker = Vec::with_capacity(chunk.len() + 14);
                    marker.extend_from_slice(b"ICC_PROFILE\0");
                    marker.extend_from_slice(&[(index + 1) as u8, count]);
                    marker.extend_from_slice(chunk);
                    ffi::jpeg_write_marker(info, 0xe2, marker.as_ptr(), marker.len() as u32);
                }
            }
            for row in img.as_raw().chunks_exact(img.width() as usize * 3) {
                let pointer = row.as_ptr();
                if ffi::jpeg_write_scanlines(info, &pointer, 1) != 1 {
                    return Err(CompressionError::EncodeFailed(
                        "MozJPEG failed to consume an RGB scanline".into(),
                    ));
                }
            }
            ffi::jpeg_finish_compress(info);
        }
        Ok(std::mem::take(&mut compressor.destination.output.bytes))
    })
    .unwrap_or_else(|payload| {
        Err(match payload.downcast::<CompressionError>() {
            Ok(error) => *error,
            Err(_) => CompressionError::EncodeFailed("MozJPEG panicked while encoding".into()),
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageDecoder;

    fn quantization_tables(bytes: &[u8]) -> Vec<u8> {
        let mut tables = Vec::new();
        let mut offset = 2;
        while offset + 4 <= bytes.len() {
            assert_eq!(bytes[offset], 0xff);
            let marker = bytes[offset + 1];
            if marker == 0xda {
                break;
            }
            let length = u16::from_be_bytes([bytes[offset + 2], bytes[offset + 3]]) as usize;
            if marker == 0xdb {
                tables.extend_from_slice(&bytes[offset + 4..offset + 2 + length]);
            }
            offset += 2 + length;
        }
        assert!(!tables.is_empty());
        tables
    }

    #[test]
    fn adaptive_tables_depend_on_pixels_and_outputs_decode() {
        let flat = RgbImage::from_pixel(64, 64, image::Rgb([150; 3]));
        let textured = RgbImage::from_fn(64, 64, |x, y| {
            image::Rgb([
                (x * 51 + y * 13) as u8,
                (x * 3 + y * 65) as u8,
                (x * 83 + y * 37) as u8,
            ])
        });
        let options = CompressionOptions::default();
        let mut tables = Vec::new();
        for image in [&flat, &textured] {
            for adaptive in [false, true] {
                let bytes = encode(image, 75, None, &options, adaptive).unwrap();
                let decoded = image::load_from_memory(&bytes).unwrap();
                assert_eq!((decoded.width(), decoded.height()), (64, 64));
                tables.push(quantization_tables(&bytes));
            }
        }
        assert_eq!(tables[0], tables[2]);
        assert_eq!(tables[0], tables[1]);
        assert_ne!(tables[2], tables[3]);
        assert_ne!(tables[1], tables[3]);
    }

    #[test]
    fn retains_multimarker_rgb_icc_progressive_and_full_chroma() {
        let image = RgbImage::from_pixel(33, 25, image::Rgb([20, 100, 200]));
        let mut profile = vec![42_u8; 140_000];
        profile[16..20].copy_from_slice(b"RGB ");
        for adaptive in [false, true] {
            let bytes = encode(
                &image,
                80,
                Some(&profile),
                &CompressionOptions::default(),
                adaptive,
            )
            .unwrap();
            let header = turbojpeg::read_header(&bytes).unwrap();
            assert!(header.is_progressive);
            assert_eq!(header.subsamp, turbojpeg::Subsamp::None);
            let mut decoder =
                image::codecs::jpeg::JpegDecoder::new(std::io::Cursor::new(&bytes)).unwrap();
            assert_eq!(decoder.icc_profile().unwrap().unwrap(), profile);
            let decoded = image::load_from_memory(&bytes).unwrap();
            assert_eq!((decoded.width(), decoded.height()), (33, 25));
        }
    }

    #[test]
    fn bounded_output_and_native_errors_return_without_unwinding() {
        let image = RgbImage::new(64, 64);
        let mut options = CompressionOptions::default();
        options.limits.max_working_memory_bytes = 8;
        assert!(matches!(
            encode(&image, 75, None, &options, false),
            Err(CompressionError::ResourceLimitExceeded(_))
        ));
        assert!(matches!(
            encode(
                &RgbImage::new(65_536, 1),
                75,
                None,
                &CompressionOptions::default(),
                false
            ),
            Err(CompressionError::EncodeFailed(_))
        ));
    }
}
