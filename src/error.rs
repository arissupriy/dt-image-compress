#[derive(Debug, thiserror::Error)]
pub enum CompressionError {
    #[error("invalid options: {0}")]
    InvalidOptions(String),
    #[error("only static JPEG and PNG are supported")]
    UnsupportedFormat,
    #[error("unsupported operation: {0}")]
    UnsupportedOperation(String),
    #[error("invalid image: {0}")]
    InvalidInput(String),
    #[error("resource limit exceeded: {0}")]
    ResourceLimitExceeded(String),
    #[error(
        "target {target_bytes} bytes was not reached; smallest observed: {best_observed_bytes} bytes ({reason})"
    )]
    TargetUnreachable {
        target_bytes: u64,
        best_observed_bytes: u64,
        reason: String,
    },
    #[error("operation cancelled")]
    Cancelled,
    #[error("encoding failed: {0}")]
    EncodeFailed(String),
    #[error("output validation failed: {0}")]
    ValidationFailed(String),
}

pub(crate) fn decode_error(e: image::ImageError) -> CompressionError {
    if matches!(e, image::ImageError::Limits(_)) {
        CompressionError::ResourceLimitExceeded(e.to_string())
    } else {
        CompressionError::InvalidInput(e.to_string())
    }
}
