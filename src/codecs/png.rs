use crate::{CompressionError, CompressionOptions, MetadataPolicy, input::png_chunks};

const DISPLAY_CHUNKS: [[u8; 4]; 12] = [
    *b"tRNS", *b"bKGD", *b"gAMA", *b"cHRM", *b"iCCP", *b"sRGB", *b"sBIT", *b"cICP", *b"mDCV",
    *b"cLLI", *b"pHYs", *b"sTER",
];

pub(crate) fn optimize(
    input: &[u8],
    options: &CompressionOptions,
) -> Result<Vec<u8>, CompressionError> {
    let mut needs_stripping = false;
    png_chunks(input, |kind, _| {
        needs_stripping |= kind[0].is_ascii_lowercase() && !DISPLAY_CHUNKS.contains(kind);
        if kind == b"eXIf" && options.metadata == MetadataPolicy::RemovePrivate {
            return Err(CompressionError::UnsupportedOperation("sample-exact PNG with EXIF: use Preserve until safe orientation handling is available".into()));
        }
        if kind == b"caBX" && options.metadata == MetadataPolicy::Preserve {
            return Err(CompressionError::UnsupportedOperation(
                "signed PNG cannot preserve its signature after optimization".into(),
            ));
        }
        Ok(())
    })?;
    let mut opts = oxipng::Options::from_preset(2);
    // Never let the optimizer fall back to private original metadata merely to save bytes.
    opts.force = needs_stripping && options.metadata == MetadataPolicy::RemovePrivate;
    opts.optimize_alpha = false;
    opts.scale_16 = false;
    opts.bit_depth_reduction = false;
    opts.color_type_reduction = false;
    opts.palette_reduction = false;
    opts.grayscale_reduction = false;
    opts.max_decompressed_size = Some(
        usize::try_from(options.limits.max_working_memory_bytes / 4)
            .map_err(|_| CompressionError::ResourceLimitExceeded("PNG allocation limit".into()))?,
    );
    opts.timeout = Some(std::time::Duration::from_secs(10));
    opts.strip = match options.metadata {
        MetadataPolicy::Preserve => oxipng::StripChunks::None,
        MetadataPolicy::RemovePrivate => {
            oxipng::StripChunks::Keep(DISPLAY_CHUNKS.into_iter().collect())
        }
    };
    oxipng::optimize_from_memory(input, &opts)
        .map_err(|e| CompressionError::EncodeFailed(e.to_string()))
}
