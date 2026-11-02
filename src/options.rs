use crate::CompressionError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    KeepOriginal,
    Jpeg,
    Png,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionMode {
    Lossless,
    Lossy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataPolicy {
    RemovePrivate,
    Preserve,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JpegBackend {
    ImageBaseline,
    Turbo444,
    Turbo420,
}

#[derive(Debug, Clone)]
pub struct ProcessingLimits {
    pub max_input_bytes: u64,
    pub max_pixels: u64,
    /// Conservative working-set estimate, not an allocator/process hard limit.
    pub max_working_memory_bytes: u64,
    pub max_encode_attempts: u32,
}

impl Default for ProcessingLimits {
    fn default() -> Self {
        Self {
            max_input_bytes: 32 * 1024 * 1024,
            max_pixels: 20_000_000,
            max_working_memory_bytes: 256 * 1024 * 1024,
            max_encode_attempts: 32,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CompressionOptions {
    pub format: OutputFormat,
    pub mode: CompressionMode,
    pub target_bytes: Option<u64>,
    pub allow_resize: bool,
    pub min_long_edge: u32,
    pub alpha_background: Option<[u8; 3]>,
    pub metadata: MetadataPolicy,
    pub limits: ProcessingLimits,
    pub jpeg_backend: JpegBackend,
    #[cfg(feature = "experimental-prefilter")]
    pub prefilter: Option<crate::experimental::PrefilterOptions>,
}

impl Default for CompressionOptions {
    fn default() -> Self {
        Self {
            format: OutputFormat::KeepOriginal,
            mode: CompressionMode::Lossy,
            target_bytes: None,
            allow_resize: false,
            min_long_edge: 1024,
            alpha_background: None,
            metadata: MetadataPolicy::RemovePrivate,
            limits: ProcessingLimits::default(),
            jpeg_backend: JpegBackend::Turbo444,
            #[cfg(feature = "experimental-prefilter")]
            prefilter: None,
        }
    }
}

impl CompressionOptions {
    pub fn validate(&self) -> Result<(), CompressionError> {
        #[cfg(feature = "experimental-prefilter")]
        if let Some(config) = self.prefilter {
            config.validate()?;
        }
        if self.target_bytes == Some(0)
            || self.min_long_edge == 0
            || self.limits.max_input_bytes == 0
            || self.limits.max_pixels == 0
            || self.limits.max_working_memory_bytes == 0
            || self.limits.max_encode_attempts == 0
        {
            return Err(CompressionError::InvalidOptions(
                "targets and limits must be positive".into(),
            ));
        }
        Ok(())
    }
}
