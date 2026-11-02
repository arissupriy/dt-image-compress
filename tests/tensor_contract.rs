#![cfg(feature = "experimental-prefilter")]

use dt_image_compress::experimental::analyze;
use image::{Rgb, RgbImage};

#[test]
fn linear_ramps_have_the_expected_energy_and_gradient_orientation() {
    for (dx, dy, energy, angle) in [
        (8, 0, 0.0009842368, 0.0),
        (0, 8, 0.0009842368, std::f32::consts::FRAC_PI_2),
        (4, 4, 0.0004921184, std::f32::consts::FRAC_PI_4),
    ] {
        let image = RgbImage::from_fn(31, 31, |x, y| {
            let value = (x * dx + y * dy) as u8;
            Rgb([value; 3])
        });
        let map = analyze(&image).unwrap();
        let center = map[15 * 31 + 15];
        assert!((center.energy - energy).abs() < 1e-7);
        assert!((center.orientation - angle).abs() < 1e-5);
        assert!(center.coherence > 0.999);
    }
}

#[test]
fn a_corner_has_energy_in_two_directions_instead_of_rank_one_coherence() {
    let image = RgbImage::from_fn(33, 33, |x, y| {
        Rgb([if x >= 16 && y >= 16 { 255 } else { 0 }; 3])
    });
    let map = analyze(&image).unwrap();
    let corner = map[16 * 33 + 16];
    assert!(corner.energy > 0.01);
    assert!((0.0..0.95).contains(&corner.coherence));
    assert!((corner.orientation - std::f32::consts::FRAC_PI_4).abs() < 1e-5);
}
