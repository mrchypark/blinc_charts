import importlib.util
import unittest
from pathlib import Path


SCRIPT_PATH = Path(__file__).resolve().parents[1] / "scripts" / "bench_ci.py"
SPEC = importlib.util.spec_from_file_location("bench_ci", SCRIPT_PATH)
bench_ci = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(bench_ci)


class CompareToBaselineTests(unittest.TestCase):
    def test_ignores_tiny_absolute_slowdowns_well_under_budget(self):
        item = {
            "id": "line_render_warm_64k",
            "absolute_fail_ns": 8_000_000,
            "regression_fail_pct": 25.0,
        }

        comparison = bench_ci.compare_to_baseline(item, 36_676.0, {"line_render_warm_64k": 23_404.0})

        self.assertIsNotNone(comparison)
        self.assertFalse(comparison["regression_failed"])

    def test_still_fails_meaningful_regressions(self):
        item = {
            "id": "multi_line_pan_plot_10_frames_1k_series_8k",
            "absolute_fail_ns": 220_000_000,
            "regression_fail_pct": 25.0,
        }

        comparison = bench_ci.compare_to_baseline(
            item,
            18_000_000.0,
            {"multi_line_pan_plot_10_frames_1k_series_8k": 8_000_000.0},
        )

        self.assertIsNotNone(comparison)
        self.assertTrue(comparison["regression_failed"])


if __name__ == "__main__":
    unittest.main()
