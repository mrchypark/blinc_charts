use blinc_core::Color;

pub fn qualitative(index: usize, alpha: f32) -> Color {
    const HUES: &[(f32, f32, f32)] = &[
        (0.35, 0.65, 1.0),
        (0.95, 0.55, 0.35),
        (0.40, 0.85, 0.55),
        (0.90, 0.75, 0.25),
        (0.75, 0.55, 0.95),
        (0.25, 0.80, 0.85),
        (0.95, 0.40, 0.60),
        (0.55, 0.82, 0.28),
    ];
    let (r, g, b) = HUES[index % HUES.len()];
    Color::rgba(r, g, b, alpha.clamp(0.0, 1.0))
}

pub fn sequential_blue(t: f32, alpha: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let r = 0.12 + 0.60 * t;
    let g = 0.22 + 0.58 * t;
    let b = 0.35 + 0.60 * t;
    Color::rgba(r, g, b, alpha.clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qualitative_is_deterministic() {
        let a = qualitative(3, 0.8);
        let b = qualitative(3, 0.8);
        assert_eq!(a, b);
    }

    #[test]
    fn sequential_blue_clamps_input() {
        let c0 = sequential_blue(-1.0, 2.0);
        let c1 = sequential_blue(0.0, 1.0);
        assert_eq!(c0, c1);
    }
}
