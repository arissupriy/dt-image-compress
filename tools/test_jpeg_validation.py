import sys
import unittest
from pathlib import Path
from jpeg_validation import launch_worker, make_fixtures


class MeasurementTests(unittest.TestCase):
    @unittest.skipUnless(sys.platform == 'linux', 'ru_maxrss units are Linux-specific')
    def test_peak_rss_is_per_child_not_cumulative(self):
        large = launch_worker([sys.executable, '-c', 'data=bytearray(64*1024*1024)'], Path('/tmp'))
        small = launch_worker([sys.executable, '-c', 'data=bytearray(1024)'], Path('/tmp'))
        self.assertEqual(large['returncode'], 0)
        self.assertGreater(large['peak_rss_kib'], small['peak_rss_kib']+20*1024)

    def test_child_failure_is_preserved(self):
        failed = launch_worker([sys.executable, '-c', 'import sys;sys.exit(7)'], Path('/tmp'))
        self.assertEqual(failed['returncode'], 7)

    def test_controlled_fixtures_contain_text_and_exact_ramp_endpoints(self):
        font = Path('/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf')
        if not font.exists(): self.skipTest('provide DejaVuSans for fixture generation')
        fixtures = make_fixtures(font)
        self.assertEqual(set(fixtures), {'document', 'gradient'})
        import numpy as np
        doc = np.asarray(fixtures['document']['image'])
        self.assertEqual(doc.shape, (768, 1024, 3))
        dark = (doc[20:200, 20:1004].mean(axis=2) < 128).mean()
        self.assertGreater(dark, 0.01); self.assertLess(dark, 0.3)
        gradient = fixtures['gradient']['image']
        self.assertEqual(gradient.getpixel((0, 20)), (0, 0, 0))
        self.assertEqual(gradient.getpixel((1023, 20)), (255, 255, 255))


if __name__ == '__main__':
    unittest.main()
