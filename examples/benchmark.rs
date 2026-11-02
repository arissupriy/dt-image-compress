use dt_image_compress::*;
use image::{ImageDecoder, ImageEncoder, Rgb, RgbImage, Rgba, RgbaImage};
use std::{
    error::Error,
    fs,
    ops::ControlFlow,
    path::{Path, PathBuf},
    time::Instant,
};

fn synthetic(name: &str) -> RgbImage {
    let mut state = 42_u32;
    RgbImage::from_fn(512, 384, |x, y| {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        let noise = (state >> 24) as u8;
        let base = match name {
            "document" => {
                let in_text = (32..480).contains(&x) && (32..352).contains(&y);
                let glyph = (x - x.min(32)) % 12 < 8 && (y - y.min(32)) % 20 < 12;
                if in_text && glyph && ((x % 12 < 2) || (y % 20 < 3) || (x % 12 > 5 && y % 20 > 7))
                {
                    0
                } else {
                    245 + noise % 11
                }
            }
            "gradient" => (x * 180 / 512 + y * 60 / 384) as u8,
            "sensor-noise" => ((x * 180 / 512 + y * 40 / 384) as u8).saturating_add(noise % 35),
            _ => {
                if (x / 3 + y / 3) % 2 == 0 {
                    32 + noise % 8
                } else {
                    220 + noise % 8
                }
            }
        };
        Rgb([
            base,
            base.saturating_add(if name == "document" { 0 } else { 5 }),
            base,
        ])
    })
}

fn psnr(source: &RgbImage, decoded: &RgbImage, roi: bool) -> f64 {
    assert_eq!(source.dimensions(), decoded.dimensions());
    let mut sum = 0.0;
    let mut count = 0_u64;
    for (a, b) in source.pixels().zip(decoded.pixels()) {
        if roi && a[0] > 64 {
            continue;
        }
        for c in 0..3 {
            sum += (f64::from(a[c]) - f64::from(b[c])).powi(2);
            count += 1;
        }
    }
    if count == 0 {
        return f64::NAN;
    }
    if sum == 0.0 {
        f64::INFINITY
    } else {
        10.0 * (255.0_f64.powi(2) / (sum / count as f64)).log10()
    }
}

fn linux_peak_rss_kib(status: &str) -> Option<u64> {
    status
        .lines()
        .find(|line| line.starts_with("VmHWM:"))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()
}

fn jpeg_samples(
    repetitions: usize,
    directory: &Path,
    samples: &Path,
) -> Result<(), Box<dyn Error>> {
    let mut paths: Vec<_> = fs::read_dir(samples)?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<Result<_, _>>()?;
    paths.retain(|p| {
        p.is_file()
            && p.extension().is_some_and(|ext| {
                ext.eq_ignore_ascii_case("jpg") || ext.eq_ignore_ascii_case("jpeg")
            })
    });
    paths.sort();
    if paths.is_empty() {
        return Err("no JPEG samples found".into());
    }
    println!(
        "sample,method,iteration,input_bytes,output_bytes,target_bytes,elapsed_ms,psnr_rgb,text_pixel_psnr,quality,encode_attempts,status,width,height,resized,best_observed_bytes"
    );
    for (index, path) in paths.iter().enumerate() {
        let name = format!("sample-{:02}", index + 1);
        let input = fs::read(path)?;
        let info = inspect_input(&input, &Default::default())?;
        if info.format != OutputFormat::Jpeg {
            return Err("expected JPEG sample".into());
        }
        let mut decoder = image::codecs::jpeg::JpegDecoder::new(std::io::Cursor::new(&input))?;
        let orientation = decoder.orientation()?;
        let mut reference = image::DynamicImage::from_decoder(decoder)?;
        reference.apply_orientation(orientation);
        let reference = reference.into_rgb8();
        for target in [200_000, 500_000, 1_000_000] {
            let base = CompressionOptions {
                jpeg_backend: JpegBackend::ImageBaseline,
                target_bytes: Some(target),
                ..Default::default()
            };
            let mut resize = base.clone();
            resize.allow_resize = true;
            let methods = vec![("B0", base.clone()), ("B1-resize", resize.clone())];
            #[cfg(feature = "experimental-prefilter")]
            let methods = {
                let mut methods = methods;
                let mut tensor = base;
                tensor.prefilter = Some(Default::default());
                resize.prefilter = Some(Default::default());
                methods.extend([("E1-tensor", tensor), ("E1-tensor-resize", resize)]);
                methods
            };
            for (method, options) in methods {
                eprintln!("{name} target={target} method={method}");
                match compress(&input, &options, |_| ControlFlow::Continue(())) {
                    Ok(_) | Err(CompressionError::TargetUnreachable { .. }) => (),
                    Err(error) => return Err(error.into()),
                }
                for iteration in 0..repetitions {
                    let begin = Instant::now();
                    let result = compress(&input, &options, |_| ControlFlow::Continue(()));
                    let elapsed = begin.elapsed().as_secs_f64() * 1000.0;
                    match result {
                        Ok(out) => {
                            assert!(out.bytes.len() as u64 <= target);
                            let decoded = image::load_from_memory(&out.bytes)?.to_rgb8();
                            assert_eq!(decoded.dimensions(), out.report.final_dimensions);
                            let restored;
                            let display = if decoded.dimensions() == reference.dimensions() {
                                &decoded
                            } else {
                                restored = image::imageops::resize(
                                    &decoded,
                                    reference.width(),
                                    reference.height(),
                                    image::imageops::FilterType::Lanczos3,
                                );
                                &restored
                            };
                            let score = psnr(&reference, display, false);
                            println!(
                                "{name},{method},{iteration},{},{},{target},{elapsed:.4},{score:.4},nan,{},{},ok,{},{},{},",
                                input.len(),
                                out.bytes.len(),
                                out.report.quality.unwrap(),
                                out.report.encode_attempts,
                                decoded.width(),
                                decoded.height(),
                                out.report.resized
                            );
                            if iteration == 0 {
                                fs::write(
                                    directory.join(format!("{name}-{target}-{method}.jpg")),
                                    out.bytes,
                                )?;
                            }
                        }
                        Err(CompressionError::TargetUnreachable {
                            best_observed_bytes,
                            ..
                        }) => {
                            println!(
                                "{name},{method},{iteration},{},,{target},{elapsed:.4},nan,nan,,,unreachable,,,false,{best_observed_bytes}",
                                input.len()
                            );
                        }
                        Err(error) => return Err(error.into()),
                    }
                }
            }
        }
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let repetitions: usize = args.next().unwrap_or("5".into()).parse()?;
    let directory = PathBuf::from(args.next().unwrap_or("benchmark-results".into()));
    fs::create_dir_all(&directory)?;
    if repetitions == 0 {
        return Err("repetitions must be positive".into());
    }
    if let Some(samples) = args.next() {
        jpeg_samples(repetitions, &directory, Path::new(&samples))?;
        if let Ok(status) = fs::read_to_string("/proc/self/status")
            && let Some(peak) = linux_peak_rss_kib(&status)
        {
            fs::write(directory.join("peak-rss-kib.txt"), format!("{peak}\n"))?;
        }
        return Ok(());
    }
    println!(
        "sample,method,iteration,input_bytes,output_bytes,target_bytes,elapsed_ms,psnr_rgb,text_pixel_psnr,quality,encode_attempts"
    );
    for name in ["document", "gradient", "sensor-noise", "fine-texture"] {
        let source = synthetic(name);
        let mut input = Vec::new();
        image::codecs::png::PngEncoder::new(&mut input).write_image(
            source.as_raw(),
            512,
            384,
            image::ExtendedColorType::Rgb8,
        )?;
        fs::write(directory.join(format!("{name}-input.png")), &input)?;
        let mut reference = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut reference, 60)
            .encode_image(&source)?;
        let base = CompressionOptions {
            jpeg_backend: JpegBackend::ImageBaseline,
            format: OutputFormat::Jpeg,
            target_bytes: Some(reference.len() as u64),
            ..Default::default()
        };
        let methods = vec![("baseline", base.clone())];
        #[cfg(feature = "experimental-prefilter")]
        let methods = {
            let mut methods = methods;
            let mut options = base.clone();
            options.prefilter = Some(experimental::PrefilterOptions::default());
            methods.push(("tensor", options));
            methods
        };
        for (method, options) in methods {
            compress(&input, &options, |_| ControlFlow::Continue(()))?;
            for iteration in 0..repetitions {
                let begin = Instant::now();
                let out = compress(&input, &options, |_| ControlFlow::Continue(()))?;
                let elapsed = begin.elapsed().as_secs_f64() * 1000.0;
                assert!(out.bytes.len() <= reference.len());
                let decoded = image::load_from_memory(&out.bytes)?.to_rgb8();
                assert_eq!(decoded.dimensions(), source.dimensions());
                let text_psnr = if name == "document" {
                    psnr(&source, &decoded, true)
                } else {
                    f64::NAN
                };
                println!(
                    "{name},{method},{iteration},{},{},{},{elapsed:.4},{:.4},{text_psnr:.4},{},{}",
                    input.len(),
                    out.bytes.len(),
                    reference.len(),
                    psnr(&source, &decoded, false),
                    out.report.quality.unwrap(),
                    out.report.encode_attempts
                );
                if iteration == 0 {
                    fs::write(directory.join(format!("{name}-{method}.jpg")), out.bytes)?;
                }
            }
        }
    }
    let rgba = RgbaImage::from_fn(128, 128, |x, y| {
        Rgba([x as u8, y as u8, 123, if x < 64 { 0 } else { 255 }])
    });
    let mut input = Vec::new();
    image::codecs::png::PngEncoder::new(&mut input).write_image(
        rgba.as_raw(),
        128,
        128,
        image::ExtendedColorType::Rgba8,
    )?;
    fs::write(directory.join("transparent-input.png"), &input)?;
    let options = CompressionOptions {
        mode: CompressionMode::Lossless,
        ..Default::default()
    };
    for iteration in 0..repetitions {
        let begin = Instant::now();
        let out = compress(&input, &options, |_| ControlFlow::Continue(()))?;
        let elapsed = begin.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(image::load_from_memory(&out.bytes)?.to_rgba8(), rgba);
        println!(
            "transparent,png-lossless,{iteration},{},{},0,{elapsed:.4},inf,nan,,1",
            input.len(),
            out.bytes.len()
        );
        if iteration == 0 {
            fs::write(directory.join("transparent-output.png"), out.bytes)?;
        }
    }
    if let Ok(status) = fs::read_to_string("/proc/self/status")
        && let Some(peak) = linux_peak_rss_kib(&status)
    {
        fs::write(directory.join("peak-rss-kib.txt"), format!("{peak}\n"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn psnr_reference_values() {
        let a = RgbImage::from_pixel(1, 1, Rgb([0, 0, 0]));
        assert!(psnr(&a, &a, false).is_infinite());
        let b = RgbImage::from_pixel(1, 1, Rgb([255, 255, 255]));
        assert_eq!(psnr(&a, &b, false), 0.0);
    }

    #[test]
    #[should_panic]
    fn psnr_rejects_mismatched_dimensions() {
        psnr(&RgbImage::new(2, 1), &RgbImage::new(1, 1), false);
    }

    #[test]
    fn rss_parser_handles_present_missing_and_invalid_values() {
        assert_eq!(
            linux_peak_rss_kib("Name: bench\nVmHWM:\t12345 kB\n"),
            Some(12345)
        );
        assert_eq!(linux_peak_rss_kib("Name: bench\n"), None);
        assert_eq!(linux_peak_rss_kib("VmHWM: invalid kB"), None);
    }
}
