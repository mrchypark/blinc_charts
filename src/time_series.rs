use std::sync::Arc;

use blinc_core::Point;

/// A compact time series with `f32` x/y values.
///
/// Assumptions for performance:
/// - `x` is sorted ascending
/// - `x.len() == y.len()`
#[derive(Clone, Debug)]
pub struct TimeSeriesF32 {
    pub x: Arc<[f32]>,
    pub y: Arc<[f32]>,
}

impl TimeSeriesF32 {
    pub fn new(x: Vec<f32>, y: Vec<f32>) -> anyhow::Result<Self> {
        anyhow::ensure!(!x.is_empty(), "x cannot be empty");
        anyhow::ensure!(x.len() == y.len(), "x/y length mismatch");
        anyhow::ensure!(
            x.windows(2).all(|w| w[0] <= w[1]),
            "x must be sorted ascending"
        );
        Ok(Self {
            x: x.into(),
            y: y.into(),
        })
    }

    pub fn len(&self) -> usize {
        self.x.len()
    }

    pub fn is_empty(&self) -> bool {
        self.x.is_empty()
    }

    pub fn point(&self, idx: usize) -> Point {
        Point::new(self.x[idx], self.y[idx])
    }

    pub fn x_min_max(&self) -> (f32, f32) {
        (*self.x.first().unwrap(), *self.x.last().unwrap())
    }

    pub fn y_min_max(&self) -> (f32, f32) {
        let mut min = f32::INFINITY;
        let mut max = f32::NEG_INFINITY;
        for &v in self.y.iter() {
            // Ignore NaN/Inf so domains stay valid and we don't propagate NaNs.
            if v.is_finite() {
                min = min.min(v);
                max = max.max(v);
            }
        }

        // If there were no finite values, return an "empty" range that won't
        // influence min/max aggregation in callers.
        if !min.is_finite() || !max.is_finite() {
            return (f32::INFINITY, f32::NEG_INFINITY);
        }
        (min, max)
    }

    pub fn lower_bound_x(&self, x: f32) -> usize {
        self.x.partition_point(|v| *v < x)
    }

    pub fn upper_bound_x(&self, x: f32) -> usize {
        self.x.partition_point(|v| *v <= x)
    }

    /// Return (nearest_index, nearest_x, nearest_y) to a given x (domain units).
    pub fn nearest_by_x(&self, x: f32) -> Option<(usize, f32, f32)> {
        if self.is_empty() {
            return None;
        }
        let i = self.lower_bound_x(x);
        if i == 0 {
            return Some((0, self.x[0], self.y[0]));
        }
        if i >= self.len() {
            let j = self.len() - 1;
            return Some((j, self.x[j], self.y[j]));
        }
        let a = i - 1;
        let b = i;
        let da = (self.x[a] - x).abs();
        let db = (self.x[b] - x).abs();
        let j = if db < da { b } else { a };
        Some((j, self.x[j], self.y[j]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_match_expected_indices() {
        let x = vec![0.0, 1.0, 1.0, 2.0, 10.0];
        let y = vec![0.0; x.len()];
        let s = TimeSeriesF32::new(x, y).unwrap();

        assert_eq!(s.lower_bound_x(-1.0), 0);
        assert_eq!(s.upper_bound_x(-1.0), 0);

        assert_eq!(s.lower_bound_x(0.0), 0);
        assert_eq!(s.upper_bound_x(0.0), 1);

        assert_eq!(s.lower_bound_x(1.0), 1);
        assert_eq!(s.upper_bound_x(1.0), 3);

        assert_eq!(s.lower_bound_x(1.5), 3);
        assert_eq!(s.upper_bound_x(1.5), 3);

        assert_eq!(s.lower_bound_x(10.0), 4);
        assert_eq!(s.upper_bound_x(10.0), 5);

        assert_eq!(s.lower_bound_x(100.0), 5);
        assert_eq!(s.upper_bound_x(100.0), 5);
    }

    #[test]
    fn y_min_max_ignores_non_finite() {
        let x = vec![0.0, 1.0, 2.0, 3.0];
        let y = vec![1.0, f32::NAN, 2.0, f32::INFINITY];
        let s = TimeSeriesF32::new(x, y).unwrap();
        assert_eq!(s.y_min_max(), (1.0, 2.0));
    }

    #[test]
    fn y_min_max_all_non_finite_returns_zero_range() {
        let x = vec![0.0, 1.0, 2.0];
        let y = vec![f32::NAN, f32::INFINITY, f32::NEG_INFINITY];
        let s = TimeSeriesF32::new(x, y).unwrap();
        assert_eq!(s.y_min_max(), (f32::INFINITY, f32::NEG_INFINITY));
    }
}
