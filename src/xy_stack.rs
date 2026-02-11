use std::sync::{Arc, Mutex};

use blinc_core::{Brush, Color, DrawContext, Rect};
use blinc_layout::canvas::canvas;
use blinc_layout::element::CursorStyle;
use blinc_layout::stack::stack;
use blinc_layout::ElementBuilder;

use crate::input::{ChartInputBindings, DragAction};
use crate::link::ChartLinkHandle;
use crate::view::ChartView;

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
}

fn drag_action(bindings: ChartInputBindings, e: &blinc_layout::event_handler::EventContext) -> DragAction {
    if bindings.brush_drag.required.matches(e.shift, e.ctrl, e.alt, e.meta) {
        return bindings.brush_drag.action;
    }
    if bindings.pan_drag.required.matches(e.shift, e.ctrl, e.alt, e.meta) {
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
                m.on_mouse_move(e.local_x, e.local_y, e.bounds_width, e.bounds_height);
                blinc_layout::stateful::request_redraw();
            }
        })
        .on_mouse_down(move |e| {
            if let Ok(mut m) = model_down.lock() {
                let brush_mod = bindings
                    .brush_drag
                    .required
                    .matches(e.shift, e.ctrl, e.alt, e.meta);
                m.on_mouse_down(brush_mod, e.local_x, e.bounds_width, e.bounds_height);
                blinc_layout::stateful::request_redraw();
            }
        })
        .on_scroll(move |e| {
            if let Ok(mut m) = model_scroll.lock() {
                m.on_scroll(e.scroll_delta_y, e.local_x, e.bounds_width, e.bounds_height);
                blinc_layout::stateful::request_redraw();
            }
        })
        .on_pinch(move |e| {
            if let Ok(mut m) = model_pinch.lock() {
                m.on_pinch(e.pinch_scale, e.local_x, e.bounds_width, e.bounds_height);
                blinc_layout::stateful::request_redraw();
            }
        })
        .on_drag(move |e| {
            if let Ok(mut m) = model_drag.lock() {
                let action = if m.is_brushing() {
                    DragAction::BrushX
                } else {
                    drag_action(bindings, e)
                };
                match action {
                    DragAction::None => {}
                    DragAction::PanX => m.on_drag_pan_total(e.drag_delta_x, e.bounds_width, e.bounds_height),
                    DragAction::BrushX => m.on_drag_brush_x_total(e.drag_delta_x, e.bounds_width, e.bounds_height),
                }
                blinc_layout::stateful::request_redraw();
            }
        })
        .on_mouse_up(move |e| {
            if let Ok(mut m) = model_up.lock() {
                let _ = m.on_mouse_up_finish_brush_x(e.bounds_width, e.bounds_height);
                m.on_drag_end();
                blinc_layout::stateful::request_redraw();
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
                m.on_mouse_move(e.local_x, e.local_y, e.bounds_width, e.bounds_height);

                // Publish hover x in domain units (or None when out of plot).
                let (px, _py, pw, _ph) = m.plot_rect(e.bounds_width, e.bounds_height);
                if pw > 0.0 && m.crosshair_x_mut().is_some() {
                    let x = m.view().px_to_x(e.local_x.clamp(px, px + pw), px, pw);
                    l.set_hover_x(Some(x));
                } else {
                    l.set_hover_x(None);
                }
                blinc_layout::stateful::request_redraw();
            }
        })
        .on_mouse_down(move |e| {
            if let (Ok(_l), Ok(mut m)) = (link_down.lock(), model_down.lock()) {
                let brush_mod = bindings
                    .brush_drag
                    .required
                    .matches(e.shift, e.ctrl, e.alt, e.meta);
                m.on_mouse_down(brush_mod, e.local_x, e.bounds_width, e.bounds_height);
                blinc_layout::stateful::request_redraw();
            }
        })
        .on_scroll(move |e| {
            if let (Ok(mut l), Ok(mut m)) = (link_scroll.lock(), model_scroll.lock()) {
                m.view_mut().domain.x = l.x_domain;
                m.on_scroll(e.scroll_delta_y, e.local_x, e.bounds_width, e.bounds_height);
                l.set_x_domain(m.view().domain.x);
                blinc_layout::stateful::request_redraw();
            }
        })
        .on_pinch(move |e| {
            if let (Ok(mut l), Ok(mut m)) = (link_pinch.lock(), model_pinch.lock()) {
                m.view_mut().domain.x = l.x_domain;
                m.on_pinch(e.pinch_scale, e.local_x, e.bounds_width, e.bounds_height);
                l.set_x_domain(m.view().domain.x);
                blinc_layout::stateful::request_redraw();
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

                match action {
                    DragAction::None => {}
                    DragAction::PanX => {
                        m.on_drag_pan_total(e.drag_delta_x, e.bounds_width, e.bounds_height);
                        l.set_x_domain(m.view().domain.x);
                    }
                    DragAction::BrushX => {
                        m.on_drag_brush_x_total(e.drag_delta_x, e.bounds_width, e.bounds_height);
                    }
                }

                blinc_layout::stateful::request_redraw();
            }
        })
        .on_mouse_up(move |e| {
            if let (Ok(mut l), Ok(mut m)) = (link_up.lock(), model_up.lock()) {
                m.view_mut().domain.x = l.x_domain;
                if let Some(sel) = m.on_mouse_up_finish_brush_x(e.bounds_width, e.bounds_height) {
                    l.set_selection_x(Some(sel));
                }
                m.on_drag_end();
                blinc_layout::stateful::request_redraw();
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
                        draw_link_selection_x(ctx, m.view(), m.plot_rect(bounds.width, bounds.height), sel);
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

