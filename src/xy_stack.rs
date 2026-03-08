use std::sync::{Arc, Mutex};

use blinc_core::{Brush, Color, DrawContext, Rect};
use blinc_layout::canvas::canvas;
use blinc_layout::element::CursorStyle;
use blinc_layout::stack::stack;
use blinc_layout::ElementBuilder;

use crate::input::{ChartInputBindings, DragAction};
use crate::link::ChartLinkHandle;
use crate::view::{ChartView, Domain1D};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChartDamage {
    None,
    Overlay,
    Plot,
}

impl ChartDamage {
    pub(crate) fn needs_redraw(self) -> bool {
        !matches!(self, Self::None)
    }
}

pub(crate) fn plot_damage(prev_domain: Domain1D, next_domain: Domain1D) -> ChartDamage {
    if prev_domain != next_domain {
        ChartDamage::Plot
    } else {
        ChartDamage::None
    }
}

/// Common behaviors for X-only interactive charts (time-series style).
///
/// This is intentionally close to d3rs's pattern: the interaction state is kept
/// on the model, and UI events simply mutate model state + request a redraw.
pub(crate) trait InteractiveXChartModel: Send + 'static {
    fn on_mouse_move(&mut self, local_x: f32, local_y: f32, w: f32, h: f32);
    fn on_mouse_down(&mut self, brush_modifier: bool, local_x: f32, w: f32, h: f32);
    fn on_scroll(&mut self, delta_y: f32, cursor_x_px: f32, w: f32, h: f32);
    fn on_pinch(&mut self, scale_delta: f32, cursor_x_px: f32, w: f32, h: f32);
    fn on_drag_pan_total(&mut self, drag_total_dx: f32, w: f32, h: f32);
    fn on_drag_brush_x_total(&mut self, drag_total_dx: f32, w: f32, h: f32);
    fn on_mouse_up_finish_brush_x(&mut self, w: f32, h: f32) -> Option<(f32, f32)>;
    fn on_drag_end(&mut self);

    fn render_plot(&mut self, ctx: &mut dyn DrawContext, w: f32, h: f32);
    fn render_overlay(&mut self, ctx: &mut dyn DrawContext, w: f32, h: f32);

    fn plot_rect(&self, w: f32, h: f32) -> (f32, f32, f32, f32);
    fn view(&self) -> &ChartView;
    fn view_mut(&mut self) -> &mut ChartView;
    fn crosshair_x_mut(&mut self) -> &mut Option<f32>;

    fn is_brushing(&self) -> bool;

    fn mouse_move_damage(&mut self, local_x: f32, local_y: f32, w: f32, h: f32) -> ChartDamage {
        self.on_mouse_move(local_x, local_y, w, h);
        ChartDamage::Overlay
    }

    fn mouse_down_damage(
        &mut self,
        brush_modifier: bool,
        local_x: f32,
        w: f32,
        h: f32,
    ) -> ChartDamage {
        self.on_mouse_down(brush_modifier, local_x, w, h);
        if brush_modifier {
            ChartDamage::Overlay
        } else {
            ChartDamage::None
        }
    }

    fn scroll_damage(&mut self, delta_y: f32, cursor_x_px: f32, w: f32, h: f32) -> ChartDamage {
        self.on_scroll(delta_y, cursor_x_px, w, h);
        ChartDamage::Plot
    }

    fn pinch_damage(&mut self, scale_delta: f32, cursor_x_px: f32, w: f32, h: f32) -> ChartDamage {
        self.on_pinch(scale_delta, cursor_x_px, w, h);
        ChartDamage::Plot
    }

    fn drag_pan_damage(&mut self, drag_total_dx: f32, w: f32, h: f32) -> ChartDamage {
        self.on_drag_pan_total(drag_total_dx, w, h);
        ChartDamage::Plot
    }

    fn drag_brush_damage(&mut self, drag_total_dx: f32, w: f32, h: f32) -> ChartDamage {
        self.on_drag_brush_x_total(drag_total_dx, w, h);
        if self.is_brushing() {
            ChartDamage::Overlay
        } else {
            ChartDamage::None
        }
    }

    fn mouse_up_damage(&mut self, w: f32, h: f32) -> (ChartDamage, Option<(f32, f32)>) {
        let had_brush = self.is_brushing();
        let selection = self.on_mouse_up_finish_brush_x(w, h);
        let damage = if had_brush {
            ChartDamage::Overlay
        } else {
            ChartDamage::None
        };
        (damage, selection)
    }
}

fn drag_action(
    bindings: ChartInputBindings,
    e: &blinc_layout::event_handler::EventContext,
) -> DragAction {
    if bindings
        .brush_drag
        .required
        .matches(e.shift, e.ctrl, e.alt, e.meta)
    {
        return bindings.brush_drag.action;
    }
    if bindings
        .pan_drag
        .required
        .matches(e.shift, e.ctrl, e.alt, e.meta)
    {
        return bindings.pan_drag.action;
    }
    DragAction::None
}

fn draw_link_selection_x(
    ctx: &mut dyn DrawContext,
    view: &ChartView,
    plot_rect: (f32, f32, f32, f32),
    selection_x: (f32, f32),
) {
    let (px, py, pw, ph) = plot_rect;
    if pw <= 0.0 || ph <= 0.0 {
        return;
    }

    let (a, b) = selection_x;
    let xa = view.x_to_px(a, px, pw);
    let xb = view.x_to_px(b, px, pw);
    let x0 = xa.min(xb).clamp(px, px + pw);
    let x1 = xa.max(xb).clamp(px, px + pw);
    ctx.fill_rect(
        Rect::new(x0, py, (x1 - x0).max(1.0), ph),
        0.0.into(),
        Brush::Solid(Color::rgba(1.0, 1.0, 1.0, 0.06)),
    );
}

fn apply_link_hover_x<M: InteractiveXChartModel>(
    model: &mut M,
    hover_x: Option<f32>,
    w: f32,
    h: f32,
) {
    let (px, _py, pw, _ph) = model.plot_rect(w, h);
    if pw <= 0.0 {
        *model.crosshair_x_mut() = None;
        return;
    }

    if let Some(hx) = hover_x {
        *model.crosshair_x_mut() = Some(model.view().x_to_px(hx, px, pw));
    } else {
        *model.crosshair_x_mut() = None;
    }
}

pub(crate) fn x_chart<M: InteractiveXChartModel>(
    handle: Arc<Mutex<M>>,
    bindings: ChartInputBindings,
) -> impl ElementBuilder {
    let model_plot = handle.clone();
    let model_overlay = handle.clone();

    let model_move = handle.clone();
    let model_scroll = handle.clone();
    let model_pinch = handle.clone();
    let model_down = handle.clone();
    let model_drag = handle.clone();
    let model_up = handle.clone();
    let model_drag_end = handle.clone();

    stack()
        .w_full()
        .h_full()
        .overflow_clip()
        .cursor(CursorStyle::Crosshair)
        .on_mouse_move(move |e| {
            if let Ok(mut m) = model_move.lock() {
                let damage =
                    m.mouse_move_damage(e.local_x, e.local_y, e.bounds_width, e.bounds_height);
                if damage.needs_redraw() {
                    blinc_layout::stateful::request_redraw();
                }
            }
        })
        .on_mouse_down(move |e| {
            if let Ok(mut m) = model_down.lock() {
                let brush_mod = bindings
                    .brush_drag
                    .required
                    .matches(e.shift, e.ctrl, e.alt, e.meta);
                let damage =
                    m.mouse_down_damage(brush_mod, e.local_x, e.bounds_width, e.bounds_height);
                if damage.needs_redraw() {
                    blinc_layout::stateful::request_redraw();
                }
            }
        })
        .on_scroll(move |e| {
            if let Ok(mut m) = model_scroll.lock() {
                let damage =
                    m.scroll_damage(e.scroll_delta_y, e.local_x, e.bounds_width, e.bounds_height);
                if damage.needs_redraw() {
                    blinc_layout::stateful::request_redraw();
                }
            }
        })
        .on_pinch(move |e| {
            if let Ok(mut m) = model_pinch.lock() {
                let damage =
                    m.pinch_damage(e.pinch_scale, e.local_x, e.bounds_width, e.bounds_height);
                if damage.needs_redraw() {
                    blinc_layout::stateful::request_redraw();
                }
            }
        })
        .on_drag(move |e| {
            if let Ok(mut m) = model_drag.lock() {
                let action = if m.is_brushing() {
                    DragAction::BrushX
                } else {
                    drag_action(bindings, e)
                };
                let damage = match action {
                    DragAction::None => ChartDamage::None,
                    DragAction::PanX => {
                        m.drag_pan_damage(e.drag_delta_x, e.bounds_width, e.bounds_height)
                    }
                    DragAction::BrushX => {
                        m.drag_brush_damage(e.drag_delta_x, e.bounds_width, e.bounds_height)
                    }
                };
                if damage.needs_redraw() {
                    blinc_layout::stateful::request_redraw();
                }
            }
        })
        .on_mouse_up(move |e| {
            if let Ok(mut m) = model_up.lock() {
                let (damage, _selection) = m.mouse_up_damage(e.bounds_width, e.bounds_height);
                m.on_drag_end();
                if damage.needs_redraw() {
                    blinc_layout::stateful::request_redraw();
                }
            }
        })
        .on_drag_end(move |_e| {
            if let Ok(mut m) = model_drag_end.lock() {
                m.on_drag_end();
            }
        })
        .child(
            canvas(move |ctx, bounds| {
                if let Ok(mut m) = model_plot.lock() {
                    m.render_plot(ctx, bounds.width, bounds.height);
                }
            })
            .w_full()
            .h_full(),
        )
        .child(
            canvas(move |ctx, bounds| {
                if let Ok(mut m) = model_overlay.lock() {
                    m.render_overlay(ctx, bounds.width, bounds.height);
                }
            })
            .w_full()
            .h_full()
            .foreground(),
        )
}

pub(crate) fn linked_x_chart<M: InteractiveXChartModel>(
    handle: Arc<Mutex<M>>,
    link: ChartLinkHandle,
    bindings: ChartInputBindings,
) -> impl ElementBuilder {
    let model_plot = handle.clone();
    let model_overlay = handle.clone();

    let model_move = handle.clone();
    let model_scroll = handle.clone();
    let model_pinch = handle.clone();
    let model_down = handle.clone();
    let model_drag = handle.clone();
    let model_up = handle.clone();
    let model_drag_end = handle.clone();

    let link_move = link.clone();
    let link_scroll = link.clone();
    let link_pinch = link.clone();
    let link_down = link.clone();
    let link_drag = link.clone();
    let link_up = link.clone();
    let link_plot = link.clone();
    let link_overlay = link.clone();

    stack()
        .w_full()
        .h_full()
        .overflow_clip()
        .cursor(CursorStyle::Crosshair)
        .on_mouse_move(move |e| {
            if let (Ok(mut l), Ok(mut m)) = (link_move.lock(), model_move.lock()) {
                m.view_mut().domain.x = l.x_domain;
                let damage =
                    m.mouse_move_damage(e.local_x, e.local_y, e.bounds_width, e.bounds_height);

                // Publish hover x in domain units (or None when out of plot).
                let (px, _py, pw, _ph) = m.plot_rect(e.bounds_width, e.bounds_height);
                let prev_hover = l.hover_x;
                let next_hover = if pw > 0.0 && m.crosshair_x_mut().is_some() {
                    Some(m.view().px_to_x(e.local_x.clamp(px, px + pw), px, pw))
                } else {
                    None
                };
                l.set_hover_x(next_hover);
                if damage.needs_redraw() || prev_hover != next_hover {
                    blinc_layout::stateful::request_redraw();
                }
            }
        })
        .on_mouse_down(move |e| {
            if let (Ok(_l), Ok(mut m)) = (link_down.lock(), model_down.lock()) {
                let brush_mod = bindings
                    .brush_drag
                    .required
                    .matches(e.shift, e.ctrl, e.alt, e.meta);
                let damage =
                    m.mouse_down_damage(brush_mod, e.local_x, e.bounds_width, e.bounds_height);
                if damage.needs_redraw() {
                    blinc_layout::stateful::request_redraw();
                }
            }
        })
        .on_scroll(move |e| {
            if let (Ok(mut l), Ok(mut m)) = (link_scroll.lock(), model_scroll.lock()) {
                m.view_mut().domain.x = l.x_domain;
                let damage =
                    m.scroll_damage(e.scroll_delta_y, e.local_x, e.bounds_width, e.bounds_height);
                l.set_x_domain(m.view().domain.x);
                if damage.needs_redraw() {
                    blinc_layout::stateful::request_redraw();
                }
            }
        })
        .on_pinch(move |e| {
            if let (Ok(mut l), Ok(mut m)) = (link_pinch.lock(), model_pinch.lock()) {
                m.view_mut().domain.x = l.x_domain;
                let damage =
                    m.pinch_damage(e.pinch_scale, e.local_x, e.bounds_width, e.bounds_height);
                l.set_x_domain(m.view().domain.x);
                if damage.needs_redraw() {
                    blinc_layout::stateful::request_redraw();
                }
            }
        })
        .on_drag(move |e| {
            if let (Ok(mut l), Ok(mut m)) = (link_drag.lock(), model_drag.lock()) {
                m.view_mut().domain.x = l.x_domain;
                let action = if m.is_brushing() {
                    DragAction::BrushX
                } else {
                    drag_action(bindings, e)
                };

                let damage = match action {
                    DragAction::None => ChartDamage::None,
                    DragAction::PanX => {
                        let damage =
                            m.drag_pan_damage(e.drag_delta_x, e.bounds_width, e.bounds_height);
                        l.set_x_domain(m.view().domain.x);
                        damage
                    }
                    DragAction::BrushX => {
                        m.drag_brush_damage(e.drag_delta_x, e.bounds_width, e.bounds_height)
                    }
                };

                if damage.needs_redraw() {
                    blinc_layout::stateful::request_redraw();
                }
            }
        })
        .on_mouse_up(move |e| {
            if let (Ok(mut l), Ok(mut m)) = (link_up.lock(), model_up.lock()) {
                m.view_mut().domain.x = l.x_domain;
                let (damage, selection) = m.mouse_up_damage(e.bounds_width, e.bounds_height);
                if let Some(sel) = selection {
                    l.set_selection_x(Some(sel));
                }
                m.on_drag_end();
                if damage.needs_redraw() {
                    blinc_layout::stateful::request_redraw();
                }
            }
        })
        .on_drag_end(move |_e| {
            if let Ok(mut m) = model_drag_end.lock() {
                m.on_drag_end();
            }
        })
        .child(
            canvas(move |ctx, bounds| {
                if let (Ok(l), Ok(mut m)) = (link_plot.lock(), model_plot.lock()) {
                    m.view_mut().domain.x = l.x_domain;
                    m.render_plot(ctx, bounds.width, bounds.height);
                }
            })
            .w_full()
            .h_full(),
        )
        .child(
            canvas(move |ctx, bounds| {
                if let (Ok(l), Ok(mut m)) = (link_overlay.lock(), model_overlay.lock()) {
                    m.view_mut().domain.x = l.x_domain;

                    if let Some(sel) = l.selection_x {
                        draw_link_selection_x(
                            ctx,
                            m.view(),
                            m.plot_rect(bounds.width, bounds.height),
                            sel,
                        );
                    }

                    apply_link_hover_x(&mut *m, l.hover_x, bounds.width, bounds.height);
                    m.render_overlay(ctx, bounds.width, bounds.height);
                }
            })
            .w_full()
            .h_full()
            .foreground(),
        )
}

#[cfg(test)]
mod tests {
    use super::{plot_damage, ChartDamage};
    use crate::view::Domain1D;

    #[test]
    fn chart_damage_helpers_report_domain_changes() {
        let domain = Domain1D::new(0.0, 10.0);
        let shifted = Domain1D::new(1.0, 11.0);

        assert_eq!(plot_damage(domain, domain), ChartDamage::None);
        assert_eq!(plot_damage(domain, shifted), ChartDamage::Plot);
    }
}
