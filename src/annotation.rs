use blinc_core::{Brush, Color, DrawContext, Point, Rect, Stroke, TextStyle};

#[derive(Clone, Debug)]
pub enum ChartAnnotation {
    Line {
        from: Point,
        to: Point,
        color: Color,
        width: f32,
    },
    Rect {
        min: Point,
        max: Point,
        color: Color,
    },
    Text {
        at: Point,
        text: String,
        color: Color,
    },
}

pub fn draw_annotations<F>(
    ctx: &mut dyn DrawContext,
    annotations: &[ChartAnnotation],
    map_point: F,
    fallback_text_color: Color,
) where
    F: Fn(Point) -> Point,
{
    for ann in annotations {
        match ann {
            ChartAnnotation::Line {
                from,
                to,
                color,
                width,
            } => {
                let p0 = map_point(*from);
                let p1 = map_point(*to);
                ctx.stroke_polyline(
                    &[p0, p1],
                    &Stroke::new((*width).max(0.75)),
                    Brush::Solid(*color),
                );
            }
            ChartAnnotation::Rect { min, max, color } => {
                let p0 = map_point(*min);
                let p1 = map_point(*max);
                let x = p0.x.min(p1.x);
                let y = p0.y.min(p1.y);
                let w = (p1.x - p0.x).abs().max(1.0);
                let h = (p1.y - p0.y).abs().max(1.0);
                ctx.fill_rect(
                    Rect::new(x, y, w, h),
                    0.0.into(),
                    Brush::Solid(Color::rgba(color.r, color.g, color.b, 0.12)),
                );
                ctx.stroke_rect(
                    Rect::new(x, y, w, h),
                    0.0.into(),
                    &Stroke::new(1.0),
                    Brush::Solid(*color),
                );
            }
            ChartAnnotation::Text { at, text, color } => {
                let p = map_point(*at);
                let style = TextStyle::new(11.0).with_color(if color.a > 0.0 {
                    *color
                } else {
                    fallback_text_color
                });
                ctx.draw_text(text, p, &style);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use blinc_core::{RecordingContext, Size};

    #[test]
    fn draw_annotations_smoke() {
        let anns = vec![
            ChartAnnotation::Line {
                from: Point::new(0.0, 0.0),
                to: Point::new(1.0, 1.0),
                color: Color::rgba(1.0, 0.0, 0.0, 1.0),
                width: 1.0,
            },
            ChartAnnotation::Rect {
                min: Point::new(0.2, 0.2),
                max: Point::new(0.8, 0.8),
                color: Color::rgba(0.0, 1.0, 0.0, 1.0),
            },
            ChartAnnotation::Text {
                at: Point::new(0.5, 0.5),
                text: "A".to_string(),
                color: Color::rgba(1.0, 1.0, 1.0, 1.0),
            },
        ];
        let mut ctx = RecordingContext::new(Size::new(100.0, 80.0));
        draw_annotations(
            &mut ctx,
            &anns,
            |p| Point::new(p.x * 100.0, p.y * 80.0),
            Color::rgba(1.0, 1.0, 1.0, 1.0),
        );
    }
}
