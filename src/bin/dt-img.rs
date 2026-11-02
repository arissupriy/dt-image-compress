use clap::{Parser, ValueEnum};
use dt_image_compress::*;
use std::{
    error::Error,
    fs::File,
    io::{self, Read, Write},
    ops::ControlFlow,
    path::{Path, PathBuf},
    process::ExitCode,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Parser)]
#[command(
    version,
    about = "Offline JPEG compression and sample-exact PNG optimization"
)]
struct Args {
    /// Input file, or - for stdin
    input: PathBuf,
    /// New output file, or - for stdout. Existing files are never overwritten.
    #[arg(short, long)]
    output: PathBuf,
    /// Maximum encoded size; explicit units: B, KB, KiB, MB, MiB
    #[arg(long, value_parser = parse_size)]
    target: Option<u64>,
    #[arg(long, value_enum, default_value = "keep")]
    format: Format,
    /// Defaults to lossless for PNG, lossy for JPEG
    #[arg(long, value_enum)]
    mode: Option<Mode>,
    #[arg(long)]
    allow_resize: bool,
    #[arg(long, default_value_t = 1024)]
    min_long_edge: u32,
    /// Explicit RGB background when flattening transparency to JPEG, e.g. FFFFFF
    #[arg(long, value_parser = parse_background)]
    background: Option<[u8; 3]>,
    #[arg(long)]
    preserve_metadata: bool,
    #[arg(long, value_enum, default_value = "turbo444")]
    jpeg_backend: Backend,
    /// Opt-in tiled structure-tensor prefilter; experimental
    #[cfg(feature = "experimental-prefilter")]
    #[arg(long)]
    prefilter: bool,
    /// Couple experimental tensor filter strength to the byte target
    #[cfg(feature = "experimental-prefilter")]
    #[arg(long, conflicts_with = "prefilter")]
    adaptive_prefilter: bool,
}

#[derive(Clone, Copy, ValueEnum)]
enum Format {
    Keep,
    Jpeg,
    Png,
}
#[derive(Clone, Copy, ValueEnum)]
enum Mode {
    Lossless,
    Lossy,
}

#[derive(Clone, Copy, ValueEnum)]
enum Backend {
    Image,
    Turbo444,
    Turbo420,
}

fn parse_size(s: &str) -> Result<u64, String> {
    for (suffix, multiplier) in [
        ("KiB", 1024),
        ("MiB", 1024 * 1024),
        ("KB", 1000),
        ("MB", 1_000_000),
        ("B", 1),
    ] {
        if let Some(number) = s.strip_suffix(suffix) {
            if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
                break;
            }
            return number
                .parse::<u64>()
                .ok()
                .and_then(|n| n.checked_mul(multiplier))
                .filter(|n| *n > 0)
                .ok_or_else(|| "size must be positive and fit in u64".into());
        }
    }
    Err("use an integer with B, KB, KiB, MB or MiB (case-sensitive)".into())
}

fn parse_background(s: &str) -> Result<[u8; 3], String> {
    let s = s.strip_prefix('#').unwrap_or(s);
    if s.len() != 6 || !s.is_ascii() {
        return Err("expected six RGB hex digits".into());
    }
    let values: Result<Vec<_>, _> = (0..3)
        .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16))
        .collect();
    values
        .map(|v| [v[0], v[1], v[2]])
        .map_err(|_| "expected six RGB hex digits".into())
}

fn cancellable_io<T: Send + 'static>(
    cancelled: &AtomicBool,
    action: impl FnOnce() -> io::Result<T> + Send + 'static,
) -> Result<T, Box<dyn Error>> {
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let _ = sender.send(action());
    });
    loop {
        if cancelled.load(Ordering::Relaxed) {
            return Err(CompressionError::Cancelled.into());
        }
        match receiver.recv_timeout(std::time::Duration::from_millis(25)) {
            Ok(result) => return result.map_err(Into::into),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Err(io::Error::other("I/O worker stopped unexpectedly").into());
            }
        }
    }
}

fn run(args: Args) -> Result<(), Box<dyn Error>> {
    if args.output != Path::new("-") && args.output.try_exists()? {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "output exists; choose a new path",
        )
        .into());
    }
    let cancelled = Arc::new(AtomicBool::new(false));
    let signal = Arc::clone(&cancelled);
    ctrlc::set_handler(move || signal.store(true, Ordering::Relaxed))?;
    let mut options = CompressionOptions {
        jpeg_backend: match args.jpeg_backend {
            Backend::Image => JpegBackend::ImageBaseline,
            Backend::Turbo444 => JpegBackend::Turbo444,
            Backend::Turbo420 => JpegBackend::Turbo420,
        },
        target_bytes: args.target,
        format: match args.format {
            Format::Keep => OutputFormat::KeepOriginal,
            Format::Jpeg => OutputFormat::Jpeg,
            Format::Png => OutputFormat::Png,
        },
        allow_resize: args.allow_resize,
        min_long_edge: args.min_long_edge,
        alpha_background: args.background,
        metadata: if args.preserve_metadata {
            MetadataPolicy::Preserve
        } else {
            MetadataPolicy::RemovePrivate
        },
        ..Default::default()
    };
    #[cfg(feature = "experimental-prefilter")]
    if args.prefilter {
        options.prefilter = Some(experimental::PrefilterOptions::default());
    }
    #[cfg(feature = "experimental-prefilter")]
    if args.adaptive_prefilter {
        options.prefilter = Some(experimental::PrefilterOptions {
            mode: experimental::PrefilterMode::RateCoupledTensor,
            ..Default::default()
        });
    }
    let max_bytes = options.limits.max_input_bytes;
    let input_path = args.input;
    let input = cancellable_io(&cancelled, move || {
        let source: Box<dyn Read> = if input_path == Path::new("-") {
            Box::new(io::stdin())
        } else {
            Box::new(File::open(input_path)?)
        };
        let mut input = Vec::new();
        source
            .take(max_bytes + 1)
            .read_to_end(&mut input)
            .map(|_| input)
    })?;
    if cancelled.load(Ordering::Relaxed) {
        return Err(CompressionError::Cancelled.into());
    }
    let info = inspect_input(&input, &options)?;
    let format = if options.format == OutputFormat::KeepOriginal {
        info.format
    } else {
        options.format
    };
    options.mode = match args.mode {
        Some(Mode::Lossless) => CompressionMode::Lossless,
        Some(Mode::Lossy) => CompressionMode::Lossy,
        None if format == OutputFormat::Png => CompressionMode::Lossless,
        None => CompressionMode::Lossy,
    };
    let out = compress(&input, &options, |_| {
        if cancelled.load(Ordering::Relaxed) {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    })?;
    if cancelled.load(Ordering::Relaxed) {
        return Err(CompressionError::Cancelled.into());
    }
    if args.output == Path::new("-") {
        let bytes = out.bytes;
        cancellable_io(&cancelled, move || io::stdout().lock().write_all(&bytes))?;
    } else {
        let extension = args
            .output
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !extension.is_empty()
            && !match out.report.output_format {
                OutputFormat::Jpeg => matches!(extension.as_str(), "jpg" | "jpeg"),
                OutputFormat::Png => extension == "png",
                _ => false,
            }
        {
            return Err(CompressionError::InvalidOptions("output extension must match encoded format; choose --format explicitly when converting".into()).into());
        }
        let parent = args
            .output
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut temp = tempfile::NamedTempFile::new_in(parent)?;
        temp.write_all(&out.bytes)?;
        temp.flush()?;
        temp.as_file().sync_all()?;
        if cancelled.load(Ordering::Relaxed) {
            return Err(CompressionError::Cancelled.into());
        }
        temp.persist_noclobber(&args.output)?;
    }
    eprintln!(
        "format={:?} input_bytes={} output_bytes={} dimensions={}x{} quality={:?} encode_attempts={} resized={}",
        out.report.output_format,
        out.report.original_bytes,
        out.report.output_bytes,
        out.report.final_dimensions.0,
        out.report.final_dimensions.1,
        out.report.quality,
        out.report.encode_attempts,
        out.report.resized
    );
    for warning in out.report.warnings {
        eprintln!("warning: {warning}");
    }
    Ok(())
}

fn main() -> ExitCode {
    match run(Args::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("dt-img: {e}");
            ExitCode::from(match e.downcast_ref::<CompressionError>() {
                Some(CompressionError::Cancelled) => 130,
                Some(CompressionError::InvalidOptions(_)) => 2,
                _ => 1,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn units_are_exact_and_overflow_is_rejected() {
        assert_eq!(parse_size("200KB").unwrap(), 200_000);
        assert_eq!(parse_size("200KiB").unwrap(), 204_800);
        for invalid in [
            "0B",
            "-1KB",
            "200kb",
            "1.5KB",
            "200",
            "18446744073709551615MiB",
        ] {
            assert!(parse_size(invalid).is_err());
        }
        assert_eq!(parse_background("#ABCDEF").unwrap(), [171, 205, 239]);
        assert!(parse_background("😃xx").is_err());
    }
}
