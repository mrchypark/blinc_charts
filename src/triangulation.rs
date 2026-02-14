use blinc_core::Point;

/// Deterministic fan triangulation in input order.
///
/// This is intentionally lightweight and stable for chart overlays where only
/// triangle count metadata is needed and a full triangulation engine would be overkill.
pub fn triangulate_fan(points: &[Point]) -> Vec<[usize; 3]> {
    if points.len() < 3 {
        return Vec::new();
    }

    let idx: Vec<usize> = points
        .iter()
        .enumerate()
        .filter_map(|(i, p)| (p.x.is_finite() && p.y.is_finite()).then_some(i))
        .collect();
    if idx.len() < 3 {
        return Vec::new();
    }

    let mut out = Vec::with_capacity(idx.len().saturating_sub(2));
    for i in 1..idx.len() - 1 {
        out.push([idx[0], idx[i], idx[i + 1]]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fan_triangulates_quad_to_two_tris() {
        let pts = vec![
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            Point::new(1.0, 1.0),
            Point::new(0.0, 1.0),
        ];
        assert_eq!(triangulate_fan(&pts).len(), 2);
    }
}
