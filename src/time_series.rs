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
            min = min.min(v);
            max = max.max(v);
        }
        (min, max)
    }

    pub fn lower_bound_x(&self, x: f32) -> usize {
        let mut lo = 0usize;
        let mut hi = self.len();
        while lo < hi {
            let mid = (lo + hi) / 2;
            if self.x[mid] < x {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        lo
    }

    pub fn upper_bound_x(&self, x: f32) -> usize {
        let mut lo = 0usize;
        let mut hi = self.len();
        while lo < hi {
            let mid = (lo + hi) / 2;
            if self.x[mid] <= x {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        lo
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
