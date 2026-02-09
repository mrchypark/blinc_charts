use blinc_core::Point;

use crate::time_series::TimeSeriesF32;

/// Parameters for downsampling.
#[derive(Clone, Copy, Debug)]
pub struct DownsampleParams {
    /// Max output points (approx).
    pub max_points: usize,
}

impl Default for DownsampleParams {
    fn default() -> Self {
        Self { max_points: 4096 }
    }
}

/// Fast min/max bucket downsampling for line charts.
///
/// This is intended for interactive rendering where raw series may contain
/// millions of points but the screen only has ~W pixels.
///
/// Output points are ordered by x and attempt to preserve extrema per bucket.
pub fn downsample_min_max(
    series: &TimeSeriesF32,
    x_min: f32,
    x_max: f32,
    params: DownsampleParams,
    out: &mut Vec<Point>,
) {
    out.clear();
    if series.is_empty() || !x_min.is_finite() || !x_max.is_finite() || x_max <= x_min {
        return;
    }

    let start = series.lower_bound_x(x_min);
    let end = series.upper_bound_x(x_max);
    if end <= start + 1 {
        if start < series.len() {
            out.push(series.point(start));
        }
        if end < series.len() {
            out.push(series.point(end));
        }
        return;
    }

    let visible = end - start;
    let target = params.max_points.max(2);
    if visible <= target {
        out.reserve(visible);
        for i in start..end {
            out.push(series.point(i));
        }
        return;
    }

    // Bucket size in indices.
    let buckets = target / 2; // each bucket emits up to 2 points (min/max)
    let buckets = buckets.max(1);
    let bucket_size = (visible + buckets - 1) / buckets;

    out.reserve(target + 4);

    // Always include the first visible point.
    out.push(series.point(start));

    let mut i = start;
    while i < end {
        let b_start = i;
        let b_end = (i + bucket_size).min(end);

        let mut min_y = f32::INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        let mut min_i = b_start;
        let mut max_i = b_start;

        for j in b_start..b_end {
            let y = series.y[j];
            if y < min_y {
                min_y = y;
                min_i = j;
            }
            if y > max_y {
                max_y = y;
                max_i = j;
            }
        }

        if min_i == max_i {
            // Flat bucket.
            if min_i != start && min_i != end - 1 {
                out.push(series.point(min_i));
            }
        } else if min_i < max_i {
            out.push(series.point(min_i));
            out.push(series.point(max_i));
        } else {
            out.push(series.point(max_i));
            out.push(series.point(min_i));
        }

        i = b_end;
    }

    // Always include the last visible point.
    out.push(series.point(end - 1));

    // De-duplicate consecutive identical points (common with flat regions).
    out.dedup_by(|a, b| a.x == b.x && a.y == b.y);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downsample_keeps_order_and_bounds() {
        let x: Vec<f32> = (0..10_000).map(|i| i as f32).collect();
        let y: Vec<f32> = (0..10_000).map(|i| (i as f32).sin()).collect();
        let series = TimeSeriesF32::new(x, y).unwrap();

        let mut out = Vec::new();
        downsample_min_max(
            &series,
            100.0,
            9000.0,
            DownsampleParams { max_points: 256 },
            &mut out,
        );

        assert!(out.len() <= 256 + 8);
        assert!(out.first().unwrap().x >= 100.0);
        assert!(out.last().unwrap().x <= 9000.0);
        assert!(out.windows(2).all(|w| w[0].x <= w[1].x));
    }
}
