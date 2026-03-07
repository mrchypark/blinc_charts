use std::mem::size_of;

use blinc_core::Point;

use crate::time_series::TimeSeriesF32;

const QUERY_SLACK_POINTS: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SeriesIdentity {
    x_ptr: usize,
    y_ptr: usize,
    len: usize,
}

impl SeriesIdentity {
    pub(crate) fn new(series: &TimeSeriesF32) -> Self {
        Self {
            x_ptr: series.x.as_ptr() as usize,
            y_ptr: series.y.as_ptr() as usize,
            len: series.len(),
        }
    }
}

/// Bounded multiresolution min/max cache for immutable time series.
#[derive(Clone, Debug, Default)]
pub struct SeriesLodCache {
    levels: Vec<Vec<Point>>,
    approx_bytes: usize,
}

impl SeriesLodCache {
    pub fn build(
        series: &TimeSeriesF32,
        min_bucket: usize,
        max_levels: usize,
        max_bytes: usize,
    ) -> Self {
        if series.is_empty() || max_levels == 0 || max_bytes < size_of::<Point>() {
            return Self::default();
        }

        let mut levels = Vec::new();
        let mut approx_bytes = 0usize;
        let min_bucket = min_bucket.max(1);

        for level_idx in 0..max_levels {
            let scale = 1usize
                .checked_shl(level_idx as u32)
                .unwrap_or(usize::MAX);
            let bucket_size = min_bucket.saturating_mul(scale).max(1);
            let level = build_level(series, bucket_size);
            if level.is_empty() {
                break;
            }

            let level_bytes = level.len().saturating_mul(size_of::<Point>());
            if approx_bytes.saturating_add(level_bytes) > max_bytes {
                continue;
            }

            approx_bytes += level_bytes;
            let terminal = level.len() <= 2;
            levels.push(level);
            if terminal {
                break;
            }
        }

        Self {
            levels,
            approx_bytes,
        }
    }

    pub fn query_into(&self, x_min: f32, x_max: f32, max_points: usize, out: &mut Vec<Point>) {
        out.clear();
        if self.levels.is_empty()
            || max_points == 0
            || !x_min.is_finite()
            || !x_max.is_finite()
            || x_max < x_min
        {
            return;
        }

        let target = max_points.max(2);
        let mut best: Option<(usize, usize, usize, f64, usize)> = None;

        for (level_idx, level) in self.levels.iter().enumerate() {
            let start = lower_bound_points(level, x_min);
            let end = upper_bound_points(level, x_max);
            let count = end.saturating_sub(start);
            if count == 0 || count > target.saturating_add(QUERY_SLACK_POINTS) {
                continue;
            }

            let score = ratio_error(count, target);
            match best {
                Some((_, _, _, best_score, best_count))
                    if score > best_score
                        || (score == best_score && count <= best_count) => {}
                _ => best = Some((level_idx, start, end, score, count)),
            }
        }

        let Some((level_idx, start, end, _, _)) = best else {
            return;
        };

        if out.capacity() < end - start {
            return;
        }
        out.extend_from_slice(&self.levels[level_idx][start..end]);
    }

    pub fn approx_bytes(&self) -> usize {
        self.approx_bytes
    }
}

fn build_level(series: &TimeSeriesF32, bucket_size: usize) -> Vec<Point> {
    if series.is_empty() {
        return Vec::new();
    }

    let len = series.len();
    let bucket_size = bucket_size.max(1);
    let mut out = Vec::with_capacity(len.div_ceil(bucket_size).saturating_mul(2) + 2);
    push_point(&mut out, series.point(0));

    let mut i = 0usize;
    while i < len {
        let b_start = i;
        let b_end = (i + bucket_size).min(len);

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
            push_point(&mut out, series.point(min_i));
        } else if min_i < max_i {
            push_point(&mut out, series.point(min_i));
            push_point(&mut out, series.point(max_i));
        } else {
            push_point(&mut out, series.point(max_i));
            push_point(&mut out, series.point(min_i));
        }

        i = b_end;
    }

    push_point(&mut out, series.point(len - 1));
    out
}

fn push_point(out: &mut Vec<Point>, point: Point) {
    if out
        .last()
        .is_some_and(|last| last.x == point.x && last.y == point.y)
    {
        return;
    }
    out.push(point);
}

fn lower_bound_points(points: &[Point], x: f32) -> usize {
    points.partition_point(|p| p.x < x)
}

fn upper_bound_points(points: &[Point], x: f32) -> usize {
    points.partition_point(|p| p.x <= x)
}

fn ratio_error(count: usize, target: usize) -> f64 {
    let count = count.max(1) as f64;
    let target = target.max(1) as f64;
    if count >= target {
        count / target
    } else {
        target / count
    }
}

#[cfg(test)]
mod tests {
    use blinc_core::Point;

    use crate::TimeSeriesF32;

    use super::SeriesLodCache;

    #[test]
    fn lod_cache_query_is_ordered_and_bounded() {
        let x: Vec<f32> = (0..10_000).map(|i| i as f32).collect();
        let y: Vec<f32> = (0..10_000).map(|i| (i as f32).sin()).collect();
        let series = TimeSeriesF32::new(x, y).unwrap();
        let cache = SeriesLodCache::build(&series, 32, 8, 1 << 20);
        let mut out = Vec::with_capacity(520);

        cache.query_into(100.0, 9000.0, 512, &mut out);

        assert!(out.len() <= 520);
        assert!(out.windows(2).all(|w| w[0].x <= w[1].x));
    }

    #[test]
    fn lod_cache_respects_byte_budget() {
        let x: Vec<f32> = (0..50_000).map(|i| i as f32).collect();
        let y: Vec<f32> = (0..50_000).map(|i| (i as f32 * 0.01).sin()).collect();
        let series = TimeSeriesF32::new(x, y).unwrap();
        let cache = SeriesLodCache::build(&series, 32, 8, 1 << 20);

        assert!(cache.approx_bytes() <= 1 << 20);
    }

    #[test]
    fn lod_cache_preserves_bucket_extrema_envelope() {
        let x: Vec<f32> = (0..4096).map(|i| i as f32).collect();
        let mut y = vec![0.0; 4096];
        y[1024] = 100.0;
        y[2048] = -100.0;
        let series = TimeSeriesF32::new(x, y).unwrap();
        let cache = SeriesLodCache::build(&series, 32, 8, 1 << 20);
        let mut out = Vec::<Point>::with_capacity(136);

        cache.query_into(0.0, 4095.0, 128, &mut out);

        assert!(out.iter().any(|p| p.y >= 100.0));
        assert!(out.iter().any(|p| p.y <= -100.0));
    }

    #[test]
    fn lod_cache_keeps_coarse_levels_when_fine_ones_exceed_budget() {
        let x: Vec<f32> = (0..50_000).map(|i| i as f32).collect();
        let y: Vec<f32> = (0..50_000).map(|i| (i as f32 * 0.01).sin()).collect();
        let series = TimeSeriesF32::new(x, y).unwrap();
        let cache = SeriesLodCache::build(&series, 32, 8, 512);
        let mut out = Vec::<Point>::with_capacity(72);

        cache.query_into(0.0, 49_999.0, 64, &mut out);

        assert!(cache.approx_bytes() <= 512);
        assert!(!out.is_empty());
    }
}
