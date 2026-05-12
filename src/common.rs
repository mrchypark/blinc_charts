use blinc_paint::{Brush, Color, CornerRadius, DrawContext, Path, PathCommand, Point, Rect};

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

pub(crate) fn closed_path_from_points(points: &[Point]) -> Option<Path> {
    closed_path_from_iter(points.iter().copied())
}

pub(crate) fn closed_path_from_iter<I>(points: I) -> Option<Path>
where
    I: IntoIterator<Item = Point>,
{
    let mut points = points.into_iter();
    let first = points.next()?;
    let (remaining, _) = points.size_hint();
    let mut commands = Vec::with_capacity(remaining.saturating_add(2));
    commands.push(PathCommand::MoveTo(first));
    commands.extend(points.map(PathCommand::LineTo));
    commands.push(PathCommand::Close);
    Some(Path::from_commands(commands))
}

#[cfg(test)]
mod tests {
    use blinc_paint::{PathCommand, Point};

    use super::{closed_path_from_iter, closed_path_from_points};

    #[test]
    fn closed_path_from_points_builds_reserved_move_lines_and_close() {
        let points = [
            Point::new(1.0, 2.0),
            Point::new(3.0, 4.0),
            Point::new(5.0, 6.0),
        ];

        let path = closed_path_from_points(&points).expect("path");

        assert_eq!(path.commands().len(), 4);
        assert!(matches!(path.commands()[0], PathCommand::MoveTo(_)));
        assert!(matches!(path.commands()[1], PathCommand::LineTo(_)));
        assert!(matches!(path.commands()[2], PathCommand::LineTo(_)));
        assert!(matches!(path.commands()[3], PathCommand::Close));
    }

    #[test]
    fn closed_path_from_iter_streams_points_without_a_slice() {
        let left = [Point::new(1.0, 1.0), Point::new(2.0, 1.0)];
        let right = [Point::new(2.0, 0.0), Point::new(1.0, 0.0)];

        let path = closed_path_from_iter(left.iter().copied().chain(right.iter().rev().copied()))
            .expect("path");

        assert_eq!(path.commands().len(), 5);
        assert!(matches!(path.commands()[0], PathCommand::MoveTo(_)));
        assert!(matches!(path.commands()[4], PathCommand::Close));
    }

    #[test]
    fn closed_path_from_points_rejects_empty_input() {
        assert!(closed_path_from_points(&[]).is_none());
    }
}
