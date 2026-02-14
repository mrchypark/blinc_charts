use blinc_core::Point;

pub fn rect_polygon(x0: f32, y0: f32, x1: f32, y1: f32) -> [Point; 4] {
    let left = x0.min(x1);
    let right = x0.max(x1);
    let top = y0.min(y1);
    let bottom = y0.max(y1);
    [
        Point::new(left, top),
        Point::new(right, top),
        Point::new(right, bottom),
        Point::new(left, bottom),
    ]
}

pub fn polygon_area(poly: &[Point]) -> f32 {
    if poly.len() < 3 {
        return 0.0;
    }
    let mut sum = 0.0f32;
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        sum += a.x * b.y - b.x * a.y;
    }
    sum.abs() * 0.5
}

pub fn point_in_polygon(p: Point, poly: &[Point]) -> bool {
    if poly.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let pi = poly[i];
        let pj = poly[j];
        let intersects = if (pi.y > p.y) != (pj.y > p.y) {
            let dy = pj.y - pi.y;
            if dy.abs() < 1e-12 {
                false
            } else {
                // Boundary convention: points exactly on an edge are treated as outside.
                p.x < (pj.x - pi.x) * (p.y - pi.y) / dy + pi.x
            }
        } else {
            false
        };
        if intersects {
            inside = !inside;
        }
        j = i;
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn area_and_contains_for_rect() {
        let poly = rect_polygon(0.0, 0.0, 2.0, 1.0);
        assert!((polygon_area(&poly) - 2.0).abs() < 1e-5);
        assert!(point_in_polygon(Point::new(1.0, 0.5), &poly));
        assert!(!point_in_polygon(Point::new(3.0, 0.5), &poly));
    }

    #[test]
    fn contains_handles_descending_non_vertical_edges() {
        let poly = vec![
            Point::new(-1.0, -1.0),
            Point::new(3.0, -1.0),
            Point::new(2.0, 1.0),
            Point::new(-2.0, 1.0),
        ];
        assert!(point_in_polygon(Point::new(1.0, 0.0), &poly));
    }
}
