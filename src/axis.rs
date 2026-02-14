use blinc_core::{Brush, Color, DrawContext, Point, Rect, TextStyle};

use crate::scale::LinearScale;
use crate::view::Domain1D;

// Fallback width used only when backend text measurement is unavailable.
const AVG_LABEL_CHAR_WIDTH_PX: f32 = 6.0;

fn label_width_px(ctx: &mut dyn DrawContext, label: &str, style: &TextStyle) -> f32 {
    let measured = ctx
        .measure_text(label, style)
        .map(|size| size.width)
        .unwrap_or(label.chars().count() as f32 * AVG_LABEL_CHAR_WIDTH_PX);
    measured.max(10.0)
}

#[derive(Clone, Debug)]
pub struct AxisTick {
    pub value: f32,
    pub px: f32,
    pub label: String,
}

pub fn build_bottom_ticks<F>(
    domain: Domain1D,
    plot_x: f32,
    plot_w: f32,
    tick_count: usize,
    formatter: F,
) -> Vec<AxisTick>
where
    F: Fn(f32) -> String,
{
    if tick_count == 0 || !domain.is_valid() || plot_w <= 0.0 {
        return Vec::new();
    }
    let s = LinearScale::new(domain.min, domain.max, plot_x, plot_x + plot_w);
    s.ticks(tick_count)
        .into_iter()
        .map(|v| AxisTick {
            value: v,
            px: s.map(v),
            label: formatter(v),
        })
        .collect()
}

pub fn build_left_ticks<F>(
    domain: Domain1D,
    plot_y: f32,
    plot_h: f32,
    tick_count: usize,
    formatter: F,
) -> Vec<AxisTick>
where
    F: Fn(f32) -> String,
{
    if tick_count == 0 || !domain.is_valid() || plot_h <= 0.0 {
        return Vec::new();
    }
    // Invert so larger values are visually higher.
    let s = LinearScale::new(domain.min, domain.max, plot_y + plot_h, plot_y);
    s.ticks(tick_count)
        .into_iter()
        .map(|v| AxisTick {
            value: v,
            px: s.map(v),
            label: formatter(v),
        })
        .collect()
}

pub fn draw_bottom_axis(
    ctx: &mut dyn DrawContext,
    ticks: &[AxisTick],
    plot_x: f32,
    plot_y: f32,
    plot_w: f32,
    axis_color: Color,
    text_color: Color,
) {
    if plot_w <= 0.0 {
        return;
    }

    ctx.fill_rect(
        Rect::new(plot_x, plot_y, plot_w, 1.0),
        0.0.into(),
        Brush::Solid(axis_color),
    );

    let style = TextStyle::new(10.0).with_color(text_color);
    for t in ticks {
        let label_w = label_width_px(ctx, &t.label, &style);
        ctx.fill_rect(
            Rect::new(t.px, plot_y, 1.0, 4.0),
            0.0.into(),
            Brush::Solid(axis_color),
        );
        ctx.draw_text(
            &t.label,
            Point::new((t.px - label_w * 0.5).max(plot_x), plot_y + 4.0),
            &style,
        );
    }
}

pub fn draw_left_axis(
    ctx: &mut dyn DrawContext,
    ticks: &[AxisTick],
    plot_x: f32,
    plot_y: f32,
    plot_h: f32,
    axis_color: Color,
    text_color: Color,
) {
    if plot_h <= 0.0 {
        return;
    }

    ctx.fill_rect(
        Rect::new(plot_x, plot_y, 1.0, plot_h),
        0.0.into(),
        Brush::Solid(axis_color),
    );

    let style = TextStyle::new(10.0).with_color(text_color);
    for t in ticks {
        let label_w = label_width_px(ctx, &t.label, &style);
        ctx.fill_rect(
            Rect::new(plot_x - 4.0, t.px, 4.0, 1.0),
            0.0.into(),
            Brush::Solid(axis_color),
        );
        ctx.draw_text(
            &t.label,
            Point::new(plot_x - 6.0 - label_w, t.px - 6.0),
            &style,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bottom_tick_builder_emits_requested_count() {
        let ticks = build_bottom_ticks(Domain1D::new(0.0, 10.0), 0.0, 100.0, 4, |v| {
            format!("{v:.0}")
        });
        assert_eq!(ticks.len(), 4);
    }

    #[test]
    fn tick_builder_with_zero_count_returns_empty() {
        let bottom = build_bottom_ticks(Domain1D::new(0.0, 10.0), 0.0, 100.0, 0, |v| {
            format!("{v:.0}")
        });
        let left = build_left_ticks(Domain1D::new(0.0, 10.0), 0.0, 100.0, 0, |v| {
            format!("{v:.0}")
        });
        assert!(bottom.is_empty());
        assert!(left.is_empty());
    }
}
