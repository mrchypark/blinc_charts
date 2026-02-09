use blinc_core::{Brush, Color, CornerRadius, DrawContext, Rect};

pub fn fill_bg(ctx: &mut dyn DrawContext, w: f32, h: f32, bg: Color) {
    ctx.fill_rect(
        Rect::new(0.0, 0.0, w, h),
        CornerRadius::default(),
        Brush::Solid(bg),
    );
}

pub fn draw_grid(
    ctx: &mut dyn DrawContext,
    plot_x: f32,
    plot_y: f32,
    plot_w: f32,
    plot_h: f32,
    grid: Color,
    grid_n: usize,
) {
    if plot_w <= 0.0 || plot_h <= 0.0 {
        return;
    }

    let grid_n = grid_n.max(1);
    for i in 0..=grid_n {
        let t = i as f32 / grid_n as f32;
        let x = plot_x + t * plot_w;
        let y = plot_y + t * plot_h;
        ctx.fill_rect(
            Rect::new(x, plot_y, 1.0, plot_h),
            0.0.into(),
            Brush::Solid(grid),
        );
        ctx.fill_rect(
            Rect::new(plot_x, y, plot_w, 1.0),
            0.0.into(),
            Brush::Solid(grid),
        );
    }
}
