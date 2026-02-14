use blinc_core::Point;

pub fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

pub fn lerp_point(a: Point, b: Point, t: f32) -> Point {
    Point::new(lerp_f32(a.x, b.x, t), lerp_f32(a.y, b.y, t))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lerp_clamps_t() {
        assert_eq!(lerp_f32(0.0, 10.0, -1.0), 0.0);
        assert_eq!(lerp_f32(0.0, 10.0, 2.0), 10.0);
    }
}
