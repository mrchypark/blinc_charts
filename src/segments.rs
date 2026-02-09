use blinc_core::Point;

/// Return contiguous runs of points where adjacent `x` deltas do not exceed `gap_dx`.
///
/// This is used to "break" lines for missing data (e.g. a time series sampled every
/// 5 minutes with occasional gaps). The returned ranges are half-open: `[start, end)`.
pub fn runs_by_gap(points: &[Point], gap_dx: f32, out: &mut Vec<(usize, usize)>) {
    out.clear();
    if points.len() < 2 {
        if !points.is_empty() {
            out.push((0, points.len()));
        }
        return;
    }

    let gap_dx = if gap_dx.is_finite() { gap_dx } else { f32::INFINITY };

    let mut start = 0usize;
    for i in 1..points.len() {
        let dx = points[i].x - points[i - 1].x;
        if dx > gap_dx {
            // Close current run at i (exclusive), then start a new run at i.
            if i > start {
                out.push((start, i));
            }
            start = i;
        }
    }
    if start < points.len() {
        out.push((start, points.len()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_single_segment_when_no_gaps() {
        let pts = [
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            Point::new(2.0, 0.0),
        ];
        let mut runs = Vec::new();
        runs_by_gap(&pts, 5.0, &mut runs);
        assert_eq!(runs, vec![(0, 3)]);
    }

    #[test]
    fn runs_split_on_large_dx() {
        let pts = [
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            Point::new(2.0, 0.0),
            Point::new(20.0, 0.0),
            Point::new(21.0, 0.0),
        ];
        let mut runs = Vec::new();
        runs_by_gap(&pts, 5.0, &mut runs);
        assert_eq!(runs, vec![(0, 3), (3, 5)]);
    }

    #[test]
    fn runs_handle_edge_cases() {
        let mut runs = Vec::new();
        runs_by_gap(&[], 1.0, &mut runs);
        assert!(runs.is_empty());

        let pts = [Point::new(0.0, 0.0)];
        runs_by_gap(&pts, 1.0, &mut runs);
        assert_eq!(runs, vec![(0, 1)]);
    }
}

