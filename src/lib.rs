//! Offline static JPEG compression and sample-exact PNG optimization.
//! No filesystem or UI dependencies are required by the library.
#![doc = include_str!("../README.md")]

mod codecs;
mod error;
#[cfg(feature = "experimental-prefilter")]
pub mod experimental;
mod input;
mod options;
mod solver;

pub use error::CompressionError;
pub use input::{InputInfo, inspect_input};
pub use options::*;

use image::{DynamicImage, ImageDecoder};
use std::ops::ControlFlow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressStage {
    Inspect,
    Decode,
    Prepare,
    Encode,
    Validate,
    Complete,
}

#[derive(Debug, Clone, Copy)]
pub struct ProgressEvent {
    pub stage: ProgressStage,
    pub encode_attempts: u32,
}

#[derive(Debug, Clone)]
pub struct CompressionReport {
    pub original_bytes: u64,
    pub output_bytes: u64,
    pub output_format: OutputFormat,
    pub final_dimensions: (u32, u32),
    pub quality: Option<u8>,
    pub encode_attempts: u32,
    pub resized: bool,
    pub metadata_policy: MetadataPolicy,
    pub warnings: Vec<String>,
    pub jpeg_backend: Option<JpegBackend>,
}

#[derive(Debug)]
pub struct CompressionOutput {
    pub bytes: Vec<u8>,
    pub report: CompressionReport,
}

pub fn compress<F>(
    input: &[u8],
    options: &CompressionOptions,
    mut on_progress: F,
) -> Result<CompressionOutput, CompressionError>
where
    F: FnMut(ProgressEvent) -> ControlFlow<()>,
{
    let mut emit = |stage, attempts| {
        if on_progress(ProgressEvent {
            stage,
            encode_attempts: attempts,
        })
        .is_break()
        {
            Err(CompressionError::Cancelled)
        } else {
            Ok(())
        }
    };
    options.validate()?;
    emit(ProgressStage::Inspect, 0)?;
    let info = inspect_input(input, options)?;
    let format = match options.format {
        OutputFormat::KeepOriginal => info.format,
        f => f,
    };
    #[cfg(feature = "experimental-prefilter")]
    if options.prefilter.is_some() && format != OutputFormat::Jpeg {
        return Err(CompressionError::UnsupportedOperation(
            "experimental prefilter requires JPEG output".into(),
        ));
    }
    let (bytes, quality, attempts, dimensions) = match format {
        OutputFormat::Png => {
            if info.format != OutputFormat::Png || options.mode != CompressionMode::Lossless {
                return Err(CompressionError::UnsupportedOperation(
                    "PNG v0.1 supports PNG-to-PNG lossless optimization only".into(),
                ));
            }
            emit(ProgressStage::Encode, 0)?;
            let bytes = codecs::png::optimize(input, options)?;
            (bytes, None, 1, info.dimensions)
        }
        OutputFormat::Jpeg => {
            if options.mode == CompressionMode::Lossless {
                return Err(CompressionError::UnsupportedOperation(
                    "lossless JPEG encoding".into(),
                ));
            }
            if options.metadata == MetadataPolicy::Preserve {
                return Err(CompressionError::UnsupportedOperation(
                    "JPEG recompression cannot preserve arbitrary metadata yet".into(),
                ));
            }
            if info.color_type.bits_per_pixel() / u16::from(info.color_type.channel_count()) > 8 {
                return Err(CompressionError::UnsupportedOperation(
                    "high-bit-depth to JPEG requires explicit tone mapping".into(),
                ));
            }
            if info.format == OutputFormat::Png {
                input::png_chunks(input, |kind, _| {
                    if matches!(
                        kind,
                        b"gAMA" | b"cHRM" | b"cICP" | b"mDCV" | b"cLLI" | b"eXIf"
                    ) {
                        return Err(CompressionError::UnsupportedOperation("PNG color/orientation conversion is not yet supported; optimize as PNG".into()));
                    }
                    Ok(())
                })?;
            }
            emit(ProgressStage::Decode, 0)?;
            let mut decoder = input::decoder(input, options)?;
            let orientation = decoder.orientation().map_err(error::decode_error)?;
            let icc = decoder.icc_profile().map_err(error::decode_error)?;
            if let Some(profile) = &icc
                && profile.get(16..20) != Some(b"RGB ")
            {
                return Err(CompressionError::UnsupportedOperation(
                    "only RGB ICC profiles can be retained on JPEG conversion".into(),
                ));
            }
            let mut decoded = if info.format == OutputFormat::Jpeg {
                drop(decoder);
                input::decode_jpeg(input, options)?
            } else {
                DynamicImage::from_decoder(decoder).map_err(error::decode_error)?
            };
            decoded.apply_orientation(orientation);
            let normalized_dimensions = (decoded.width(), decoded.height());
            emit(ProgressStage::Prepare, 0)?;
            let rgb = if decoded.has_alpha() {
                let background = options.alpha_background.ok_or_else(|| {
                    CompressionError::UnsupportedOperation(
                        "transparency requires alpha_background for JPEG".into(),
                    )
                })?;
                let rgba = decoded.to_rgba8();
                image::RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
                    let p = rgba.get_pixel(x, y).0;
                    image::Rgb(std::array::from_fn(|c| {
                        ((u32::from(p[c]) * u32::from(p[3])
                            + u32::from(background[c]) * (255 - u32::from(p[3]))
                            + 127)
                            / 255) as u8
                    }))
                })
            } else {
                decoded.into_rgb8()
            };
            #[cfg(feature = "experimental-prefilter")]
            let rgb = if let Some(config) = options.prefilter {
                if icc.is_some() {
                    return Err(CompressionError::UnsupportedOperation(
                        "experimental prefilter does not support ICC-managed images".into(),
                    ));
                }
                // Two RGB frames plus conservative scratch for a 260x260 tile with halo.
                if u64::from(rgb.width()) * u64::from(rgb.height()) * 6 + 8 * 1024 * 1024
                    > options.limits.max_working_memory_bytes / 2
                {
                    return Err(CompressionError::ResourceLimitExceeded(
                        "experimental scratch buffers".into(),
                    ));
                }
                experimental::prefilter_with_progress(&rgb, config, || {
                    emit(ProgressStage::Prepare, 0)
                })?
            } else {
                rgb
            };
            let floor = options.min_long_edge.min(rgb.width().max(rgb.height()));
            let mut scaled = None;
            let mut attempts = 0;
            let mut smallest = u64::MAX;
            loop {
                let working = scaled.as_ref().unwrap_or(&rgb);
                if let Some(target) = options.target_bytes {
                    let budget = options.limits.max_encode_attempts - attempts;
                    let mut encoded = 0;
                    let result = solver::search_target(target, budget, |q| {
                        emit(ProgressStage::Encode, attempts + encoded)?;
                        encoded += 1;
                        codecs::jpeg::encode(working, q, icc.as_deref(), options)
                    })?;
                    attempts += result.attempts;
                    smallest = smallest.min(result.smallest);
                    if let Some(best) = result.best {
                        break (
                            best.bytes,
                            Some(best.quality),
                            attempts,
                            working.dimensions(),
                        );
                    }
                    if !options.allow_resize
                        || attempts >= options.limits.max_encode_attempts
                        || working.width().max(working.height()) <= floor
                    {
                        return Err(CompressionError::TargetUnreachable {
                            target_bytes: target,
                            best_observed_bytes: smallest,
                            reason: "quality, dimension or encode budget reached".into(),
                        });
                    }
                    let long = working.width().max(working.height());
                    let factor =
                        ((target as f64 / result.smallest as f64).sqrt() * 0.95).clamp(0.5, 0.9);
                    let next_long = ((f64::from(long) * factor) as u32).max(floor).min(long - 1);
                    let scale = f64::from(next_long)
                        / f64::from(normalized_dimensions.0.max(normalized_dimensions.1));
                    let w = (f64::from(rgb.width()) * scale).round().max(1.0) as u32;
                    let h = (f64::from(rgb.height()) * scale).round().max(1.0) as u32;
                    emit(ProgressStage::Prepare, attempts)?;
                    scaled = Some(image::imageops::resize(
                        &rgb,
                        w,
                        h,
                        image::imageops::FilterType::Lanczos3,
                    ));
                } else {
                    emit(ProgressStage::Encode, 0)?;
                    break (
                        codecs::jpeg::encode(working, 80, icc.as_deref(), options)?,
                        Some(80),
                        1,
                        working.dimensions(),
                    );
                }
            }
        }
        OutputFormat::KeepOriginal => unreachable!(),
    };
    if let Some(target) = options.target_bytes
        && bytes.len() as u64 > target
    {
        return Err(CompressionError::TargetUnreachable {
            target_bytes: target,
            best_observed_bytes: bytes.len() as u64,
            reason: "lossless output exceeds target".into(),
        });
    }
    emit(ProgressStage::Validate, attempts)?;
    let mut verify_options = options.clone();
    verify_options.limits.max_input_bytes = options.limits.max_working_memory_bytes / 4;
    let decoder = input::decoder(&bytes, &verify_options)
        .map_err(|e| CompressionError::ValidationFailed(e.to_string()))?;
    let check_dimensions = decoder.dimensions();
    let verified = DynamicImage::from_decoder(decoder)
        .map_err(|e| CompressionError::ValidationFailed(e.to_string()))?;
    drop(verified);
    if check_dimensions != dimensions {
        return Err(CompressionError::ValidationFailed(
            "dimension mismatch".into(),
        ));
    }
    emit(ProgressStage::Complete, attempts)?;
    #[cfg(feature = "experimental-prefilter")]
    let warnings = if options.prefilter.is_some() {
        vec!["experimental prefilter changes pixels; text preservation is not guaranteed".into()]
    } else {
        vec![]
    };
    #[cfg(not(feature = "experimental-prefilter"))]
    let warnings = vec![];
    let report = CompressionReport {
        original_bytes: input.len() as u64,
        output_bytes: bytes.len() as u64,
        output_format: format,
        final_dimensions: dimensions,
        quality,
        encode_attempts: attempts,
        resized: dimensions != info.dimensions
            && !(dimensions.0 == info.dimensions.1 && dimensions.1 == info.dimensions.0),
        metadata_policy: options.metadata,
        warnings,
        jpeg_backend: (format == OutputFormat::Jpeg).then_some(options.jpeg_backend),
    };
    Ok(CompressionOutput { bytes, report })
}
