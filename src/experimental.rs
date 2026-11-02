//! Unstable, opt-in experiments. No semantic text or face recognition is implied.
use crate::CompressionError;
use image::RgbImage;

#[derive(Debug, Clone, Copy)]
pub struct TensorSample {
    pub energy: f32,
    pub coherence: f32,
    pub orientation: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct PrefilterOptions {
    pub mode: PrefilterMode,
    pub threshold: f32,
    pub coherence_boost: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefilterMode {
    FixedTensor,
    RateCoupledTensor,
    Gaussian,
}

impl Default for PrefilterOptions {
    fn default() -> Self {
        Self {
            mode: PrefilterMode::FixedTensor,
            threshold: 0.04,
            coherence_boost: 4.0,
        }
    }
}

impl PrefilterOptions {
    pub(crate) fn validate(self) -> Result<(), CompressionError> {
        if !self.threshold.is_finite()
            || self.threshold <= 0.0
            || self.threshold > 1.0
            || !self.coherence_boost.is_finite()
            || !(0.0..=16.0).contains(&self.coherence_boost)
        {
            return Err(CompressionError::InvalidOptions(
                "prefilter threshold must be in (0,1], coherence boost in [0,16]".into(),
            ));
        }
        Ok(())
    }

    pub(crate) fn for_pressure(mut self, reference_bytes: u64, target_bytes: u64) -> Self {
        let pressure = (reference_bytes as f64 / target_bytes as f64).max(1.0);
        let cap = f64::from(self.threshold).max(0.25);
        self.threshold = (f64::from(self.threshold) * pressure.sqrt()).min(cap) as f32;
        self.mode = PrefilterMode::FixedTensor;
        self
    }
}

fn sample_index(x: i32, y: i32, w: u32, h: u32) -> usize {
    y.clamp(0, h as i32 - 1) as usize * w as usize + x.clamp(0, w as i32 - 1) as usize
}

/// Scharr gradients followed by a 3x3 Gaussian window on all tensor components.
/// The weighted sRGB channel sum is a nonlinear luma proxy, not linear luminance.
pub fn analyze(img: &RgbImage) -> Result<Vec<TensorSample>, CompressionError> {
    let (w, h) = img.dimensions();
    let n = u64::from(w) * u64::from(h);
    if n == 0 {
        return Err(CompressionError::InvalidInput("empty image".into()));
    }
    // Diagnostic tensor maps are full-frame; the compression prefilter uses bounded tiles.
    if n > 1_000_000 {
        return Err(CompressionError::ResourceLimitExceeded(
            "experimental tensor is limited to 1 megapixel".into(),
        ));
    }
    let luma: Vec<f32> = img
        .pixels()
        .map(|p| {
            (0.299 * f32::from(p[0]) + 0.587 * f32::from(p[1]) + 0.114 * f32::from(p[2])) / 255.0
        })
        .collect();
    let mut raw = vec![[0_f32; 3]; n as usize];
    let scharr = [[-3.0, 0.0, 3.0], [-10.0, 0.0, 10.0], [-3.0, 0.0, 3.0]];
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let (mut gx, mut gy) = (0.0, 0.0);
            for j in 0..3 {
                for i in 0..3 {
                    let value = luma[sample_index(x + i as i32 - 1, y + j as i32 - 1, w, h)];
                    gx += scharr[j][i] * value / 32.0;
                    gy += scharr[i][j] * value / 32.0;
                }
            }
            raw[sample_index(x, y, w, h)] = [gx * gx, gx * gy, gy * gy];
        }
    }
    let mut result = Vec::with_capacity(n as usize);
    let kernel = [1.0_f32, 2.0, 1.0];
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let mut j = [0_f32; 3];
            for dy in 0..3 {
                for dx in 0..3 {
                    let local = raw[sample_index(x + dx as i32 - 1, y + dy as i32 - 1, w, h)];
                    let weight = kernel[dx] * kernel[dy] / 16.0;
                    for c in 0..3 {
                        j[c] += local[c] * weight;
                    }
                }
            }
            let energy = (j[0] + j[2]).max(0.0);
            let difference = (j[0] - j[2]).hypot(2.0 * j[1]);
            let coherence = (difference / (energy + 1e-12)).powi(2).clamp(0.0, 1.0);
            result.push(TensorSample {
                energy,
                coherence,
                orientation: 0.5 * (2.0 * j[1]).atan2(j[0] - j[2]),
            });
        }
    }
    Ok(result)
}

pub fn prefilter(img: &RgbImage, options: PrefilterOptions) -> Result<RgbImage, CompressionError> {
    prefilter_with_progress(img, options, || Ok(()))
}

pub(crate) fn prefilter_with_progress(
    img: &RgbImage,
    options: PrefilterOptions,
    mut checkpoint: impl FnMut() -> Result<(), CompressionError>,
) -> Result<RgbImage, CompressionError> {
    options.validate()?;
    let (w, h) = img.dimensions();
    let pixels = u64::from(w) * u64::from(h);
    if pixels == 0 {
        return Err(CompressionError::InvalidInput("empty image".into()));
    }
    if pixels > 50_000_000 {
        return Err(CompressionError::ResourceLimitExceeded(
            "experimental prefilter pixel count".into(),
        ));
    }
    let mut output = RgbImage::new(w, h);
    const EDGE: u32 = 256;
    // Scharr radius 1 + tensor window radius 1; blur sigma .8 has radius 1.
    const HALO: u32 = 2;
    for y in (0..h).step_by(EDGE as usize) {
        for x in (0..w).step_by(EDGE as usize) {
            checkpoint()?;
            let (x0, y0) = (x.saturating_sub(HALO), y.saturating_sub(HALO));
            let (x1, y1) = ((x + EDGE).min(w), (y + EDGE).min(h));
            let right = (x1 + HALO).min(w);
            let bottom = (y1 + HALO).min(h);
            let tile = image::imageops::crop_imm(img, x0, y0, right - x0, bottom - y0).to_image();
            let filtered = prefilter_full(&tile, options)?;
            for dy in y..y1 {
                for dx in x..x1 {
                    output.put_pixel(dx, dy, *filtered.get_pixel(dx - x0, dy - y0));
                }
            }
        }
    }
    Ok(output)
}

fn prefilter_full(img: &RgbImage, options: PrefilterOptions) -> Result<RgbImage, CompressionError> {
    if options.mode == PrefilterMode::RateCoupledTensor {
        return Err(CompressionError::InvalidOptions(
            "rate coupling requires compression target context".into(),
        ));
    }
    if options.mode == PrefilterMode::Gaussian {
        return Ok(image::imageops::blur(img, 0.8));
    }
    let tensor = analyze(img)?;
    let blurred = image::imageops::blur(img, 0.8);
    Ok(RgbImage::from_fn(img.width(), img.height(), |x, y| {
        let t = tensor[(y * img.width() + x) as usize];
        let importance = t.energy * (1.0 + options.coherence_boost * t.coherence);
        // f32 squaring can underflow for a valid tiny positive threshold.
        let importance = f64::from(importance);
        let weight = (importance / (importance + f64::from(options.threshold).powi(2))) as f32;
        image::Rgb(std::array::from_fn(|c| {
            (weight * f32::from(img.get_pixel(x, y)[c])
                + (1.0 - weight) * f32::from(blurred.get_pixel(x, y)[c]))
            .round()
            .clamp(0.0, 255.0) as u8
        }))
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgb;

    #[test]
    fn constant_field_has_zero_energy_and_is_unchanged() {
        let img = RgbImage::from_pixel(16, 16, Rgb([123, 45, 67]));
        let tensor = analyze(&img).unwrap();
        assert!(
            tensor
                .iter()
                .all(|t| t.energy < 1e-10 && t.coherence == 0.0)
        );
        assert_eq!(prefilter(&img, Default::default()).unwrap(), img);
    }

    #[test]
    fn gaussian_window_does_not_classify_all_noise_as_rank_one() {
        let mut n = 42_u32;
        let img = RgbImage::from_fn(40, 40, |_, _| {
            n = n.wrapping_mul(1664525).wrapping_add(1013904223);
            let v = (n >> 24) as u8;
            Rgb([v, v, v])
        });
        let tensor = analyze(&img).unwrap();
        let mean = tensor.iter().map(|t| t.coherence).sum::<f32>() / tensor.len() as f32;
        assert!(mean < 0.8, "mean coherence={mean}");
        assert!(tensor.iter().all(|t| t.energy >= 0.0
            && (0.0..=1.0).contains(&t.coherence)
            && t.orientation.is_finite()));
    }

    #[test]
    fn borders_and_tiny_images_are_defined() {
        for (w, h) in [(1, 1), (1, 2), (2, 1), (2, 2)] {
            let img = RgbImage::from_pixel(w, h, Rgb([255, 0, 0]));
            assert_eq!(prefilter(&img, Default::default()).unwrap(), img);
        }
    }

    #[test]
    fn strong_edge_survives_and_invalid_parameters_are_rejected() {
        let img = RgbImage::from_fn(20, 20, |x, _| {
            if x < 10 {
                Rgb([0, 0, 0])
            } else {
                Rgb([255, 255, 255])
            }
        });
        let output = prefilter(&img, Default::default()).unwrap();
        assert!(output.get_pixel(9, 10)[0] < 10);
        assert!(output.get_pixel(10, 10)[0] > 245);
        assert!(
            prefilter(
                &img,
                PrefilterOptions {
                    threshold: f32::NAN,
                    ..Default::default()
                }
            )
            .is_err()
        );
    }

    #[test]
    fn tiny_positive_threshold_does_not_turn_flat_white_black() {
        let white = RgbImage::from_pixel(3, 3, Rgb([255, 255, 255]));
        for threshold in [f32::MIN_POSITIVE, f32::from_bits(1)] {
            let output = prefilter(
                &white,
                PrefilterOptions {
                    threshold,
                    ..Default::default()
                },
            )
            .unwrap();
            assert_eq!(output, white);
        }
    }

    #[test]
    fn tile_halos_match_the_full_reference_at_seams_and_image_edges() {
        for (w, h) in [(519, 271), (1, 517), (517, 1)] {
            let mut state = 42_u32;
            let img = RgbImage::from_fn(w, h, |x, y| {
                state = state.wrapping_mul(1664525).wrapping_add(1013904223);
                Rgb([(state >> 24) as u8, (x % 256) as u8, (y % 256) as u8])
            });
            let actual = prefilter(&img, Default::default()).unwrap();
            let full = prefilter_full(&img, Default::default()).unwrap();
            assert_eq!(actual, full, "dimensions={w}x{h}");
        }
    }

    #[test]
    fn tiled_prefilter_can_cancel_between_tiles() {
        let img = RgbImage::from_pixel(300, 10, Rgb([255, 255, 255]));
        let mut tiles = 0;
        let output = prefilter_with_progress(&img, Default::default(), || {
            tiles += 1;
            if tiles == 2 {
                Err(CompressionError::Cancelled)
            } else {
                Ok(())
            }
        });
        assert!(matches!(output, Err(CompressionError::Cancelled)));
        assert_eq!(tiles, 2);
    }

    #[test]
    fn pressure_scaling_is_bounded_and_never_weakens_the_requested_threshold() {
        let base = PrefilterOptions::default();
        assert_eq!(base.for_pressure(100, 100).threshold, 0.04);
        assert_eq!(base.for_pressure(400, 100).threshold, 0.08);
        assert_eq!(base.for_pressure(u64::MAX, 1).threshold, 0.25);
        let strong = PrefilterOptions {
            threshold: 0.8,
            ..base
        };
        assert_eq!(strong.for_pressure(u64::MAX, 1).threshold, 0.8);
    }

    #[test]
    fn gaussian_control_blurs_a_color_impulse_without_tensor_protection() {
        let mut image = RgbImage::new(9, 9);
        image.put_pixel(4, 4, Rgb([255, 0, 0]));
        let blurred = prefilter(
            &image,
            PrefilterOptions {
                mode: PrefilterMode::Gaussian,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(blurred.get_pixel(4, 4)[0] < 200);
        assert!(blurred.get_pixel(3, 4)[0] > 0);
    }
}
