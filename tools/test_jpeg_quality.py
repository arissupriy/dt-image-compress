import math
import unittest

import numpy as np

from jpeg_quality import measure


def rgb(gray):
    return np.repeat(gray[..., None], 3, axis=2)


def direct_ssim(a, b):
    """Independent weighted-window reference, using centered deviations."""
    axis = np.arange(-5, 6, dtype=np.float64)
    kernel = np.exp(-axis**2 / (2 * 1.5**2))
    weights = np.outer(kernel, kernel)
    weights /= weights.sum()
    a = a.astype(float) @ np.array([0.299, 0.587, 0.114])
    b = b.astype(float) @ np.array([0.299, 0.587, 0.114])
    values = []
    for y in range(5, a.shape[0] - 5):
        for x in range(5, a.shape[1] - 5):
            u, v = a[y-5:y+6, x-5:x+6], b[y-5:y+6, x-5:x+6]
            mu, mv = (u * weights).sum(), (v * weights).sum()
            vu = (weights * (u-mu)**2).sum()
            vv = (weights * (v-mv)**2).sum()
            cov = (weights * (u-mu) * (v-mv)).sum()
            values.append(((2*mu*mv+6.5025)*(2*cov+58.5225)) /
                          ((mu**2+mv**2+6.5025)*(vu+vv+58.5225)))
    return float(np.mean(values))


class QualityTests(unittest.TestCase):
    def test_identity_including_black_and_white(self):
        for value in [0, 127, 255]:
            image = np.full((17, 23, 3), value, dtype=np.uint8)
            result = measure(image, image)
            self.assertAlmostEqual(result['ssim_y'], 1.0, places=12)
            self.assertEqual(result['psnr_rgb'], math.inf)
            self.assertEqual(result['mae_rgb'], 0)

    def test_constants_have_known_ssim_and_psnr(self):
        a = np.zeros((17, 23, 3), dtype=np.uint8)
        b = np.full_like(a, 64)
        result = measure(a, b)
        self.assertAlmostEqual(result['ssim_y'], 6.5025/(64**2+6.5025), places=12)
        self.assertAlmostEqual(result['psnr_rgb'], 20*math.log10(255/64), places=12)
        self.assertEqual(result['mae_rgb'], 64)
        self.assertEqual(result, measure(b, a))

    def test_tiles_match_independent_reference_and_do_not_modify_pixels(self):
        rng = np.random.default_rng(20261103)
        a = rng.integers(0, 256, (23, 29, 3), dtype=np.uint8)
        b = rng.integers(0, 256, a.shape, dtype=np.uint8)
        saved_a, saved_b = a.copy(), b.copy()
        expected = direct_ssim(a, b)
        for tile in [1, 7, 256]:
            self.assertAlmostEqual(measure(a, b, tile=tile)['ssim_y'], expected, places=12)
        np.testing.assert_array_equal(a, saved_a)
        np.testing.assert_array_equal(b, saved_b)

    def test_anticorrelation_is_not_clamped_away(self):
        a = rgb(((np.indices((21, 25)).sum(axis=0) % 2)*255).astype(np.uint8))
        self.assertLess(measure(a, 255-a)['ssim_y'], 0)

    def test_roi_and_validation(self):
        rng = np.random.default_rng(42)
        a = rng.integers(0, 256, (31, 37, 3), dtype=np.uint8)
        b = a.copy(); b[8:24, 7:28] //= 2
        self.assertEqual(measure(a, b, roi=(7, 8, 28, 24)), measure(a[8:24, 7:28], b[8:24, 7:28]))
        for other, roi, tile in [(a[:10], None, 256), (a, (-1, 0, 20, 20), 256),
                                 (a, (0, 0, 10, 10), 256), (a, None, 0)]:
            with self.assertRaises(ValueError): measure(a, other, roi=roi, tile=tile)


if __name__ == '__main__':
    unittest.main()
