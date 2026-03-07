use std::sync::{Arc, Mutex};

use blinc_core::{Brush, Color, CornerRadius, DrawContext, Point, Rect, Stroke, TextStyle};
use blinc_layout::ElementBuilder;

use crate::brush::BrushX;
use crate::density_map::draw_density_bins;
use crate::link::ChartLinkHandle;
use crate::lod::{downsample_min_max, DownsampleParams};
use crate::lod_cache::{SeriesIdentity, SeriesLodCache};
use crate::segments::runs_by_gap;
use crate::time_series::TimeSeriesF32;
use crate::view::{ChartView, Domain1D, Domain2D};
use crate::xy_stack::{ChartDamage, InteractiveXChartModel};

const MULTI_LINE_LOD_MIN_BUCKET: usize = 32;
const MULTI_LINE_LOD_MAX_LEVELS: usize = 8;
const MULTI_LINE_LOD_TOTAL_MAX_BYTES: usize = 64 * 1024 * 1024;
const DENSITY_OVERVIEW_MAX_CELLS_X: usize = 128;
const DENSITY_OVERVIEW_MAX_CELLS_Y: usize = 64;
const DENSITY_OVERVIEW_MAX_POINTS_PER_SERIES: usize = 32;
const DENSITY_OVERVIEW_MIN_SEGMENTS_PER_SERIES: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CacheKey {
    x_min: u32,
    x_max: u32,
    y_min: u32,
    y_max: u32,
    plot_x: u32,
    plot_y: u32,
    plot_w: u32,
    plot_h: u32,
    series_len: u32,
    series_fingerprint: u64,
    max_series: u32,
    max_total_segments: u32,
    max_points_per_series: u32,
    gap_dx: u32,
}

impl CacheKey {
    fn new(model: &MultiLineChartModel, plot: (f32, f32, f32, f32)) -> Self {
        let (px, py, pw, ph) = plot;
        Self {
            x_min: model.view.domain.x.min.to_bits(),
            x_max: model.view.domain.x.max.to_bits(),
            y_min: model.view.domain.y.min.to_bits(),
            y_max: model.view.domain.y.max.to_bits(),
            plot_x: px.to_bits(),
            plot_y: py.to_bits(),
            plot_w: pw.to_bits(),
            plot_h: ph.to_bits(),
            series_len: model.series.len() as u32,
            series_fingerprint: series_set_fingerprint(&model.series),
            max_series: model.style.max_series as u32,
            max_total_segments: model.style.max_total_segments as u32,
            max_points_per_series: model.style.max_points_per_series as u32,
            gap_dx: model.gap_dx.to_bits(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct CachedRun {
    start: usize,
    end: usize,
    series_index: usize,
}

/// Visual styling for a multi-line chart.
#[derive(Clone, Debug)]
pub struct MultiLineChartStyle {
    pub bg: Color,
    pub grid: Color,
    pub crosshair: Color,
    pub text: Color,

    pub stroke_width: f32,
    pub series_alpha: f32,
    pub scroll_zoom_factor: f32,
    pub pinch_zoom_min: f32,

    /// Maximum number of series to draw as lines.
    ///
    /// (If you want a 10k-series overview, you'll likely want a density renderer instead.)
    pub max_series: usize,

    /// Hard budget for the total number of line segments we emit per frame.
    ///
    /// This avoids overflowing the GPU line segment buffer (default ~50k).
    pub max_total_segments: usize,

    /// Cap for per-series downsample output. Actual per-series points may be lower due to
    /// `max_total_segments` budgeting.
    pub max_points_per_series: usize,
}

impl Default for MultiLineChartStyle {
    fn default() -> Self {
        Self {
            bg: Color::rgba(0.08, 0.09, 0.11, 1.0),
            grid: Color::rgba(1.0, 1.0, 1.0, 0.08),
            crosshair: Color::rgba(1.0, 1.0, 1.0, 0.35),
            text: Color::rgba(1.0, 1.0, 1.0, 0.85),
            stroke_width: 1.0,
            series_alpha: 0.18,
            scroll_zoom_factor: 0.02,
            pinch_zoom_min: 0.01,
            max_series: 1_000,
            max_total_segments: 45_000,
            max_points_per_series: 2_048,
        }
    }
}

/// Mutable model for an interactive multi-line chart.
///
/// This intentionally uses a single scratch buffer reused across series to keep memory use low.
pub struct MultiLineChartModel {
    pub series: Vec<TimeSeriesF32>,
    pub view: ChartView,
    pub style: MultiLineChartStyle,

    /// If finite, consecutive points with `dx > gap_dx` will not be connected.
    /// Use this to "break" lines when samples are missing.
    pub gap_dx: f32,

    pub crosshair_x: Option<f32>, // local px in plot area

    // EventRouter drag deltas are "offset from drag start".
    last_drag_total_x: Option<f32>,

    scratch_data: Vec<Point>, // data coords
    scratch_px: Vec<Point>,   // local px coords
    scratch_runs: Vec<(usize, usize)>,
    downsample_params: DownsampleParams,
    lod_caches: Vec<Option<SeriesLodCache>>,
    lod_cache_identities: Vec<Option<SeriesIdentity>>,
    lod_cache_failed_budget: Vec<Option<usize>>,
    lod_cache_bytes: usize,
    density_bins: Vec<u32>,

    brush_x: BrushX,

    // Cached (downsampled + transformed) geometry in local px coords.
    cached_key: Option<CacheKey>,
    cached_points_px: Vec<Point>,
    cached_runs: Vec<CachedRun>,
}

impl MultiLineChartModel {
    pub fn new(series: Vec<TimeSeriesF32>) -> anyhow::Result<Self> {
        anyhow::ensure!(
            !series.is_empty(),
            "MultiLineChartModel requires at least 1 series"
        );

        let mut x_min = f32::INFINITY;
        let mut x_max = f32::NEG_INFINITY;
        let mut y_min = f32::INFINITY;
        let mut y_max = f32::NEG_INFINITY;
        for s in &series {
            let (sx0, sx1) = s.x_min_max();
            x_min = x_min.min(sx0);
            x_max = x_max.max(sx1);
            let (sy0, sy1) = s.y_min_max();
            y_min = y_min.min(sy0);
            y_max = y_max.max(sy1);
        }

        // Avoid degenerate y ranges.
        if y_max.partial_cmp(&y_min) != Some(std::cmp::Ordering::Greater) {
            // Handle degenerate or invalid y-ranges.
            if y_min.is_finite() && y_max.is_finite() {
                y_min -= 1.0;
                y_max += 1.0;
            } else {
                // Fallback for non-finite ranges (e.g. all NaN data).
                y_min = -1.0;
                y_max = 1.0;
            }
        }

        let domain = Domain2D::new(Domain1D::new(x_min, x_max), Domain1D::new(y_min, y_max));
        let series_len = series.len();
        Ok(Self {
            series,
            view: ChartView::new(domain),
            style: MultiLineChartStyle::default(),
            gap_dx: f32::INFINITY,
            crosshair_x: None,
            last_drag_total_x: None,
            scratch_data: Vec::new(),
            scratch_px: Vec::new(),
            scratch_runs: Vec::new(),
            downsample_params: DownsampleParams::default(),
            lod_caches: std::iter::repeat_with(|| None).take(series_len).collect(),
            lod_cache_identities: std::iter::repeat_with(|| None).take(series_len).collect(),
            lod_cache_failed_budget: std::iter::repeat_with(|| None).take(series_len).collect(),
            lod_cache_bytes: 0,
            density_bins: Vec::new(),
            brush_x: BrushX::default(),
            cached_key: None,
            cached_points_px: Vec::new(),
            cached_runs: Vec::new(),
        })
    }

    pub fn set_gap_dx(&mut self, gap_dx: f32) {
        self.gap_dx = gap_dx;
    }

    fn should_use_density_overview(&self, plot_w: f32) -> bool {
        if plot_w <= 0.0 {
            return false;
        }

        let plot_segment_budget = (plot_w.ceil() as usize).saturating_mul(16);
        let per_series_segment_budget = self.style.max_total_segments / self.series.len().max(1);
        self.series.len() > self.style.max_series
            || (self.style.max_total_segments > plot_segment_budget
                && per_series_segment_budget < DENSITY_OVERVIEW_MIN_SEGMENTS_PER_SERIES)
    }

    fn plot_rect(&self, w: f32, h: f32) -> (f32, f32, f32, f32) {
        self.view.plot_rect(w, h)
    }

    fn sync_lod_caches_with_series_len(&mut self) {
        if self.lod_caches.len() > self.series.len() {
            for cache in self.lod_caches[self.series.len()..].iter().flatten() {
                self.lod_cache_bytes = self.lod_cache_bytes.saturating_sub(cache.approx_bytes());
            }
            self.lod_caches.truncate(self.series.len());
            self.lod_cache_identities.truncate(self.series.len());
            self.lod_cache_failed_budget.truncate(self.series.len());
        } else if self.lod_caches.len() < self.series.len() {
            self.lod_caches.resize_with(self.series.len(), || None);
            self.lod_cache_identities
                .resize_with(self.series.len(), || None);
            self.lod_cache_failed_budget
                .resize_with(self.series.len(), || None);
        }
    }

    fn render_density_overview(
        &mut self,
        ctx: &mut dyn DrawContext,
        px: f32,
        py: f32,
        pw: f32,
        ph: f32,
    ) {
        let bins_w = ((pw / 6.0).floor() as usize).clamp(8, DENSITY_OVERVIEW_MAX_CELLS_X);
        let bins_h = ((ph / 6.0).floor() as usize).clamp(8, DENSITY_OVERVIEW_MAX_CELLS_Y);
        self.density_bins.clear();
        self.density_bins.resize(bins_w * bins_h, 0);

        if !self.view.domain.is_valid() {
            return;
        }

        let span_x = self.view.domain.x.span();
        let span_y = self.view.domain.y.span();
        let inv_x = 1.0 / span_x.max(1e-12);
        let inv_y = 1.0 / span_y.max(1e-12);
        let density_params = DownsampleParams {
            max_points: DENSITY_OVERVIEW_MAX_POINTS_PER_SERIES,
        };
        let mut bins_max = 0u32;

        for series in &self.series {
            downsample_min_max(
                series,
                self.view.domain.x.min,
                self.view.domain.x.max,
                density_params,
                &mut self.scratch_data,
            );
            for p in &self.scratch_data {
                if !p.x.is_finite() || !p.y.is_finite() {
                    continue;
                }
                let tx = ((p.x - self.view.domain.x.min) * inv_x).clamp(0.0, 0.999_999);
                let ty = ((p.y - self.view.domain.y.min) * inv_y).clamp(0.0, 0.999_999);
                let ix = (tx * bins_w as f32) as usize;
                let iy = (ty * bins_h as f32) as usize;
                let idx = iy * bins_w + ix;
                if let Some(v) = self.density_bins.get_mut(idx) {
                    *v = v.saturating_add(1);
                    bins_max = bins_max.max(*v);
                }
            }
        }

        draw_density_bins(
            ctx,
            &self.density_bins,
            bins_w,
            bins_h,
            bins_max,
            Rect::new(px, py, pw, ph),
        );
    }

    fn query_series_lod_into_scratch(
        &mut self,
        index: usize,
        visible_series: usize,
        x_min: f32,
        x_max: f32,
        max_points: usize,
    ) -> bool {
        self.sync_lod_caches_with_series_len();

        let identity = SeriesIdentity::new(&self.series[index]);
        if self.lod_cache_identities[index] != Some(identity) {
            if let Some(cache) = self.lod_caches[index].take() {
                self.lod_cache_bytes = self.lod_cache_bytes.saturating_sub(cache.approx_bytes());
            }
            self.lod_cache_identities[index] = Some(identity);
            self.lod_cache_failed_budget[index] = None;
        }

        let raw_visible = self.series[index]
            .upper_bound_x(x_max)
            .saturating_sub(self.series[index].lower_bound_x(x_min));
        if raw_visible <= max_points.saturating_add(8) {
            return false;
        }
        let raw_start = self.series[index].lower_bound_x(x_min);
        let raw_end = self.series[index].upper_bound_x(x_max);

        if self.lod_caches[index].is_none() {
            let remaining_budget =
                MULTI_LINE_LOD_TOTAL_MAX_BYTES.saturating_sub(self.lod_cache_bytes);
            let remaining_series = visible_series.saturating_sub(index).max(1);
            let per_series_budget = remaining_budget / remaining_series;
            if per_series_budget == 0 {
                return false;
            }
            if self.lod_cache_failed_budget[index]
                .is_some_and(|failed_budget| per_series_budget <= failed_budget)
            {
                return false;
            }

            let cache = SeriesLodCache::build(
                &self.series[index],
                MULTI_LINE_LOD_MIN_BUCKET,
                MULTI_LINE_LOD_MAX_LEVELS,
                per_series_budget,
            );
            let approx_bytes = cache.approx_bytes();
            if approx_bytes == 0 {
                self.lod_cache_failed_budget[index] = Some(per_series_budget);
                return false;
            }

            self.lod_cache_bytes = self.lod_cache_bytes.saturating_add(approx_bytes);
            self.lod_caches[index] = Some(cache);
            self.lod_cache_failed_budget[index] = None;
        }

        let Some(cache) = self.lod_caches[index].as_ref() else {
            return false;
        };
        let needed = max_points.saturating_add(8);
        if self.scratch_data.capacity() < needed {
            self.scratch_data.reserve(needed - self.scratch_data.capacity());
        }
        cache.query_into(x_min, x_max, max_points, &mut self.scratch_data);
        let raw_first_x = self.series[index].x.get(raw_start).copied();
        let raw_last_x = raw_end
            .checked_sub(1)
            .and_then(|idx| self.series[index].x.get(idx))
            .copied();
        let keeps_edges = self.scratch_data.first().map(|p| p.x) == raw_first_x
            && self.scratch_data.last().map(|p| p.x) == raw_last_x;
        if !keeps_edges {
            self.scratch_data.clear();
            return false;
        }
        self.scratch_data.len() >= 2
    }

    pub fn on_mouse_move(&mut self, local_x: f32, local_y: f32, w: f32, h: f32) -> ChartDamage {
        let prev_crosshair = self.crosshair_x;
        let (px, py, pw, ph) = self.plot_rect(w, h);
        if pw <= 0.0 || ph <= 0.0 {
            self.crosshair_x = None;
            return overlay_damage(prev_crosshair, self.crosshair_x);
        }
        if local_x < px || local_x > px + pw || local_y < py || local_y > py + ph {
            self.crosshair_x = None;
            return overlay_damage(prev_crosshair, self.crosshair_x);
        }
        self.crosshair_x = Some(local_x);
        overlay_damage(prev_crosshair, self.crosshair_x)
    }

    pub fn on_scroll(&mut self, delta_y: f32, cursor_x_px: f32, w: f32, h: f32) -> ChartDamage {
        let prev_domain = self.view.domain.x;
        let (px, _py, pw, _ph) = self.plot_rect(w, h);
        if pw <= 0.0 {
            return ChartDamage::None;
        }
        let cursor_x_px = cursor_x_px.clamp(px, px + pw);
        let pivot_x = self.view.px_to_x(cursor_x_px, px, pw);

        let delta_y = delta_y.clamp(-250.0, 250.0);
        let zoom = (-delta_y * self.style.scroll_zoom_factor).exp();
        self.view.domain.x.zoom_about(pivot_x, zoom);
        self.view.domain.x.clamp_span_min(1e-6);
        plot_damage(prev_domain, self.view.domain.x)
    }

    pub fn on_pinch(&mut self, scale_delta: f32, cursor_x_px: f32, w: f32, h: f32) -> ChartDamage {
        let prev_domain = self.view.domain.x;
        let (px, _py, pw, _ph) = self.plot_rect(w, h);
        if pw <= 0.0 {
            return ChartDamage::None;
        }
        let cursor_x_px = cursor_x_px.clamp(px, px + pw);
        let pivot_x = self.view.px_to_x(cursor_x_px, px, pw);

        let zoom = scale_delta.max(self.style.pinch_zoom_min);
        self.view.domain.x.zoom_about(pivot_x, zoom);
        self.view.domain.x.clamp_span_min(1e-6);
        plot_damage(prev_domain, self.view.domain.x)
    }

    /// Pan using drag "total delta from start" (EventContext::drag_delta_x).
    pub fn on_drag_pan_total(&mut self, drag_total_dx: f32, w: f32, h: f32) -> ChartDamage {
        let prev_domain = self.view.domain.x;
        let (_px, _py, pw, _ph) = self.plot_rect(w, h);
        if pw <= 0.0 {
            return ChartDamage::None;
        }
        // Convert total-from-start to incremental delta since last event.
        let prev = self.last_drag_total_x.replace(drag_total_dx);
        let drag_dx = match prev {
            Some(p) => drag_total_dx - p,
            None => 0.0,
        };

        // Convert pixel delta to domain delta.
        let dx = -drag_dx / pw * self.view.domain.x.span();
        self.view.domain.x.pan_by(dx);
        plot_damage(prev_domain, self.view.domain.x)
    }

    pub fn on_drag_end(&mut self) {
        self.last_drag_total_x = None;
    }

    pub fn on_mouse_down(&mut self, shift: bool, local_x: f32, w: f32, h: f32) -> ChartDamage {
        let prev_range = self.brush_x.range_px();
        if !shift {
            return ChartDamage::None;
        }
        let (px, _py, pw, _ph) = self.plot_rect(w, h);
        if pw <= 0.0 {
            return ChartDamage::None;
        }
        self.brush_x.begin(local_x.clamp(px, px + pw));
        self.last_drag_total_x = None;
        overlay_range_damage(prev_range, self.brush_x.range_px())
    }

    pub fn on_drag_brush_x_total(&mut self, drag_total_dx: f32, w: f32, h: f32) -> ChartDamage {
        let prev_range = self.brush_x.range_px();
        if !self.brush_x.is_active() {
            return ChartDamage::None;
        }
        let (px, _py, pw, _ph) = self.plot_rect(w, h);
        if pw <= 0.0 {
            return ChartDamage::None;
        }
        let Some(start_x) = self.brush_x.anchor_px() else {
            return ChartDamage::None;
        };
        let x = start_x + drag_total_dx;
        self.brush_x.update(x.clamp(px, px + pw));
        overlay_range_damage(prev_range, self.brush_x.range_px())
    }

    pub fn on_mouse_up_finish_brush_x(&mut self, w: f32, h: f32) -> Option<(f32, f32)> {
        let (px, _py, pw, _ph) = self.plot_rect(w, h);
        if pw <= 0.0 {
            self.brush_x.cancel();
            return None;
        }
        let (a_px, b_px) = self.brush_x.take_final_px()?;
        let a = self.view.px_to_x(a_px, px, pw);
        let b = self.view.px_to_x(b_px, px, pw);
        Some(if a <= b { (a, b) } else { (b, a) })
    }

    fn palette_color(i: usize, alpha: f32) -> Color {
        // Golden-ratio hue step for decent distribution.
        let h = (i as f32 * 0.618_034) % 1.0;
        let s = 0.75;
        let v = 0.95;
        let (r, g, b) = hsv_to_rgb(h, s, v);
        Color::rgba(r, g, b, alpha)
    }

    pub fn render_plot(&mut self, ctx: &mut dyn DrawContext, w: f32, h: f32) {
        ctx.fill_rect(
            Rect::new(0.0, 0.0, w, h),
            CornerRadius::default(),
            Brush::Solid(self.style.bg),
        );

        let (px, py, pw, ph) = self.plot_rect(w, h);
        if pw <= 0.0 || ph <= 0.0 {
            return;
        }

        crate::common::draw_grid(ctx, px, py, pw, ph, self.style.grid, 4);
        if self.should_use_density_overview(pw) {
            self.render_density_overview(ctx, px, py, pw, ph);
            return;
        }

        self.ensure_cached_geometry(w, h);
        if self.cached_runs.is_empty() {
            return;
        }

        let stroke = Stroke::new(self.style.stroke_width);
        for run in self.cached_runs.iter().copied() {
            if run.end <= run.start + 1 || run.end > self.cached_points_px.len() {
                continue;
            }
            let color = Self::palette_color(run.series_index, self.style.series_alpha);
            ctx.stroke_polyline(
                &self.cached_points_px[run.start..run.end],
                &stroke,
                Brush::Solid(color),
            );
        }
    }

    fn ensure_cached_geometry(&mut self, w: f32, h: f32) {
        let plot = self.plot_rect(w, h);
        let (px, py, pw, ph) = plot;
        if pw <= 0.0 || ph <= 0.0 {
            self.cached_key = None;
            self.cached_points_px.clear();
            self.cached_runs.clear();
            return;
        }

        self.sync_lod_caches_with_series_len();
        let key = CacheKey::new(self, plot);
        if self.cached_key == Some(key) {
            return;
        }

        self.cached_points_px.clear();
        self.cached_runs.clear();

        let n = self.series.len().min(self.style.max_series);
        if n == 0 {
            self.cached_key = Some(key);
            return;
        }

        let mut remaining_segments = self.style.max_total_segments.max(1);
        let x_min = self.view.domain.x.min;
        let x_max = self.view.domain.x.max;

        // Per-series point cap: also bounded by pixels so we don't waste work.
        let px_cap = (pw.ceil() as usize).saturating_mul(2).clamp(64, 200_000);
        let hard_per_series_cap = self.style.max_points_per_series.max(2).min(px_cap);
        let affine = self.view.plot_affine(px, py, pw, ph);

        for si in 0..n {
            if remaining_segments == 0 {
                break;
            }

            // Budget segments fairly across remaining series.
            let remaining_series = (n - si).max(1);
            let seg_budget = (remaining_segments / remaining_series).max(8);
            let point_budget = (seg_budget + 1).clamp(2, hard_per_series_cap);

            let used_cache = self.query_series_lod_into_scratch(si, n, x_min, x_max, point_budget);
            if !used_cache {
                self.downsample_params.max_points = point_budget;
                downsample_min_max(
                    &self.series[si],
                    x_min,
                    x_max,
                    self.downsample_params,
                    &mut self.scratch_data,
                );
            }

            if self.scratch_data.len() < 2 {
                continue;
            }

            // Convert to px.
            self.scratch_px.clear();
            self.scratch_px.reserve(self.scratch_data.len());
            for p in &self.scratch_data {
                self.scratch_px.push(affine.map_point(*p));
            }

            // Split runs on missing data gaps.
            runs_by_gap(&self.scratch_data, self.gap_dx, &mut self.scratch_runs);

            for (a, b) in self.scratch_runs.iter().copied() {
                if remaining_segments == 0 {
                    break;
                }

                let len = b.saturating_sub(a);
                if len < 2 {
                    continue;
                }

                let need = len - 1;
                let end = if need > remaining_segments {
                    a + remaining_segments + 1
                } else {
                    b
                };

                if end > a + 1 && end <= b {
                    let start_idx = self.cached_points_px.len();
                    self.cached_points_px
                        .extend_from_slice(&self.scratch_px[a..end]);
                    let end_idx = self.cached_points_px.len();
                    self.cached_runs.push(CachedRun {
                        start: start_idx,
                        end: end_idx,
                        series_index: si,
                    });
                }

                if need > remaining_segments {
                    remaining_segments = 0;
                    break;
                } else {
                    remaining_segments = remaining_segments.saturating_sub(need);
                }
            }
        }

        self.cached_key = Some(key);
    }

    pub fn render_overlay(&mut self, ctx: &mut dyn DrawContext, w: f32, h: f32) {
        let (px, py, pw, ph) = self.plot_rect(w, h);
        if pw <= 0.0 || ph <= 0.0 {
            return;
        }

        if let Some((a, b)) = self.brush_x.range_px() {
            let x = a.clamp(px, px + pw);
            let w = (b - a).abs().max(1.0);
            ctx.fill_rect(
                Rect::new(x.min(px + pw), py, w.min(pw), ph),
                0.0.into(),
                Brush::Solid(Color::rgba(0.35, 0.65, 1.0, 0.10)),
            );
        }

        if let Some(cx) = self.crosshair_x {
            let x = cx.clamp(px, px + pw);
            ctx.fill_rect(
                Rect::new(x, py, 1.0, ph),
                0.0.into(),
                Brush::Solid(self.style.crosshair),
            );

            let xv = self.view.px_to_x(x, px, pw);
            let text = format!("x={:.3}", xv);
            let style = TextStyle::new(12.0).with_color(self.style.text);
            ctx.draw_text(&text, Point::new(px + 6.0, py + 6.0), &style);
        }
    }
}

impl InteractiveXChartModel for MultiLineChartModel {
    fn on_mouse_move(&mut self, local_x: f32, local_y: f32, w: f32, h: f32) {
        let _ = MultiLineChartModel::on_mouse_move(self, local_x, local_y, w, h);
    }

    fn mouse_move_damage(&mut self, local_x: f32, local_y: f32, w: f32, h: f32) -> ChartDamage {
        MultiLineChartModel::on_mouse_move(self, local_x, local_y, w, h)
    }

    fn on_mouse_down(&mut self, brush_modifier: bool, local_x: f32, w: f32, h: f32) {
        let _ = MultiLineChartModel::on_mouse_down(self, brush_modifier, local_x, w, h);
    }

    fn mouse_down_damage(
        &mut self,
        brush_modifier: bool,
        local_x: f32,
        w: f32,
        h: f32,
    ) -> ChartDamage {
        MultiLineChartModel::on_mouse_down(self, brush_modifier, local_x, w, h)
    }

    fn on_scroll(&mut self, delta_y: f32, cursor_x_px: f32, w: f32, h: f32) {
        let _ = MultiLineChartModel::on_scroll(self, delta_y, cursor_x_px, w, h);
    }

    fn scroll_damage(&mut self, delta_y: f32, cursor_x_px: f32, w: f32, h: f32) -> ChartDamage {
        MultiLineChartModel::on_scroll(self, delta_y, cursor_x_px, w, h)
    }

    fn on_pinch(&mut self, scale_delta: f32, cursor_x_px: f32, w: f32, h: f32) {
        let _ = MultiLineChartModel::on_pinch(self, scale_delta, cursor_x_px, w, h);
    }

    fn pinch_damage(&mut self, scale_delta: f32, cursor_x_px: f32, w: f32, h: f32) -> ChartDamage {
        MultiLineChartModel::on_pinch(self, scale_delta, cursor_x_px, w, h)
    }

    fn on_drag_pan_total(&mut self, drag_total_dx: f32, w: f32, h: f32) {
        let _ = MultiLineChartModel::on_drag_pan_total(self, drag_total_dx, w, h);
    }

    fn drag_pan_damage(&mut self, drag_total_dx: f32, w: f32, h: f32) -> ChartDamage {
        MultiLineChartModel::on_drag_pan_total(self, drag_total_dx, w, h)
    }

    fn on_drag_brush_x_total(&mut self, drag_total_dx: f32, w: f32, h: f32) {
        let _ = MultiLineChartModel::on_drag_brush_x_total(self, drag_total_dx, w, h);
    }

    fn drag_brush_damage(&mut self, drag_total_dx: f32, w: f32, h: f32) -> ChartDamage {
        MultiLineChartModel::on_drag_brush_x_total(self, drag_total_dx, w, h)
    }

    fn on_mouse_up_finish_brush_x(&mut self, w: f32, h: f32) -> Option<(f32, f32)> {
        MultiLineChartModel::on_mouse_up_finish_brush_x(self, w, h)
    }

    fn on_drag_end(&mut self) {
        MultiLineChartModel::on_drag_end(self);
    }

    fn render_plot(&mut self, ctx: &mut dyn DrawContext, w: f32, h: f32) {
        MultiLineChartModel::render_plot(self, ctx, w, h);
    }

    fn render_overlay(&mut self, ctx: &mut dyn DrawContext, w: f32, h: f32) {
        MultiLineChartModel::render_overlay(self, ctx, w, h);
    }

    fn plot_rect(&self, w: f32, h: f32) -> (f32, f32, f32, f32) {
        self.view.plot_rect(w, h)
    }

    fn view(&self) -> &ChartView {
        &self.view
    }

    fn view_mut(&mut self) -> &mut ChartView {
        &mut self.view
    }

    fn crosshair_x_mut(&mut self) -> &mut Option<f32> {
        &mut self.crosshair_x
    }

    fn is_brushing(&self) -> bool {
        self.brush_x.is_active()
    }
}

fn overlay_damage(prev_crosshair: Option<f32>, next_crosshair: Option<f32>) -> ChartDamage {
    if prev_crosshair != next_crosshair {
        ChartDamage::Overlay
    } else {
        ChartDamage::None
    }
}

fn overlay_range_damage(
    prev_range: Option<(f32, f32)>,
    next_range: Option<(f32, f32)>,
) -> ChartDamage {
    if prev_range != next_range {
        ChartDamage::Overlay
    } else {
        ChartDamage::None
    }
}

fn plot_damage(prev_domain: Domain1D, next_domain: Domain1D) -> ChartDamage {
    if prev_domain != next_domain {
        ChartDamage::Plot
    } else {
        ChartDamage::None
    }
}

fn series_set_fingerprint(series: &[TimeSeriesF32]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for s in series {
        hash ^= s.x.as_ptr() as u64;
        hash = hash.wrapping_mul(0x1000_0000_01b3);
        hash ^= s.y.as_ptr() as u64;
        hash = hash.wrapping_mul(0x1000_0000_01b3);
        hash ^= s.len() as u64;
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xy_stack::ChartDamage;
    use blinc_core::{RecordingContext, Size};

    #[test]
    fn new_rejects_empty_series() {
        assert!(MultiLineChartModel::new(Vec::new()).is_err());
    }

    #[test]
    fn render_plot_does_not_panic_when_max_points_per_series_is_too_small() {
        let s = TimeSeriesF32::new(vec![0.0, 1.0, 2.0], vec![1.0, 2.0, 3.0]).unwrap();
        let mut model = MultiLineChartModel::new(vec![s]).unwrap();
        model.style.max_points_per_series = 1;

        let mut ctx = RecordingContext::new(Size::new(320.0, 200.0));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            model.render_plot(&mut ctx, 320.0, 200.0);
        }));

        assert!(result.is_ok());
    }

    #[test]
    fn scroll_returns_plot_damage() {
        let series = TimeSeriesF32::new(vec![0.0, 1.0, 2.0], vec![1.0, 2.0, 3.0]).unwrap();
        let mut model = MultiLineChartModel::new(vec![series]).unwrap();

        let damage = model.on_scroll(40.0, 120.0, 320.0, 200.0);

        assert_eq!(damage, ChartDamage::Plot);
    }

    #[test]
    fn multi_line_switches_to_density_mode_when_series_budget_is_exceeded() {
        let series: Vec<TimeSeriesF32> = (0..10_000)
            .map(|_| TimeSeriesF32::new(vec![0.0, 1.0, 2.0], vec![0.0, 1.0, 0.0]).unwrap())
            .collect();
        let model = MultiLineChartModel::new(series).unwrap();

        assert!(model.should_use_density_overview(1280.0));
    }

    #[test]
    fn multi_line_switches_to_density_mode_when_segment_budget_is_exceeded() {
        let series: Vec<TimeSeriesF32> = (0..600)
            .map(|_| TimeSeriesF32::new(vec![0.0, 1.0, 2.0], vec![0.0, 1.0, 0.0]).unwrap())
            .collect();
        let mut model = MultiLineChartModel::new(series).unwrap();
        model.style.max_series = usize::MAX;
        model.style.max_total_segments = 3_000;

        assert!(model.should_use_density_overview(128.0));
    }

    #[test]
    fn render_plot_handles_series_appended_after_construction() {
        let series = TimeSeriesF32::new(vec![0.0, 1.0, 2.0], vec![1.0, 2.0, 3.0]).unwrap();
        let mut model = MultiLineChartModel::new(vec![series.clone()]).unwrap();
        model.series.push(series);

        let mut ctx = RecordingContext::new(Size::new(320.0, 200.0));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            model.render_plot(&mut ctx, 320.0, 200.0);
        }));

        assert!(result.is_ok());
    }

    #[test]
    fn render_plot_invalidates_cached_geometry_after_series_append() {
        let series = TimeSeriesF32::new(vec![0.0, 1.0, 2.0], vec![1.0, 2.0, 3.0]).unwrap();
        let mut model = MultiLineChartModel::new(vec![series.clone()]).unwrap();
        let mut ctx = RecordingContext::new(Size::new(320.0, 200.0));

        model.render_plot(&mut ctx, 320.0, 200.0);
        model.series.push(series);
        model.render_plot(&mut ctx, 320.0, 200.0);

        assert!(model.cached_runs.iter().any(|run| run.series_index == 1));
    }

    #[test]
    fn render_plot_truncates_lod_state_when_series_shrinks() {
        let series = TimeSeriesF32::new(vec![0.0, 1.0, 2.0], vec![1.0, 2.0, 3.0]).unwrap();
        let mut model = MultiLineChartModel::new(vec![series.clone(), series]).unwrap();
        let mut ctx = RecordingContext::new(Size::new(320.0, 200.0));

        model.render_plot(&mut ctx, 320.0, 200.0);
        model.series.pop();
        model.render_plot(&mut ctx, 320.0, 200.0);

        assert_eq!(model.lod_caches.len(), 1);
        assert_eq!(model.lod_cache_identities.len(), 1);
        assert_eq!(model.lod_cache_failed_budget.len(), 1);
    }
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    let h = (h.fract() + 1.0).fract() * 6.0;
    let i = h.floor() as i32;
    let f = h - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    match i.rem_euclid(6) {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    }
}

/// Shared handle for a multi-line chart model.
#[derive(Clone)]
pub struct MultiLineChartHandle(pub Arc<Mutex<MultiLineChartModel>>);

impl MultiLineChartHandle {
    pub fn new(model: MultiLineChartModel) -> Self {
        Self(Arc::new(Mutex::new(model)))
    }
}

/// Create an interactive multi-line chart element.
///
/// Interactions:
/// - Scroll/pinch: zoom X about cursor
/// - Drag: pan X
pub fn multi_line_chart(handle: MultiLineChartHandle) -> impl ElementBuilder {
    multi_line_chart_with_bindings(handle, crate::ChartInputBindings::default())
}

pub fn multi_line_chart_with_bindings(
    handle: MultiLineChartHandle,
    bindings: crate::ChartInputBindings,
) -> impl ElementBuilder {
    crate::xy_stack::x_chart(handle.0, bindings)
}

/// Create a linked multi-line chart element (shared X domain + hover + selection).
///
/// See `linked_line_chart` for behavioral details; this mirrors the same linking behavior.
pub fn linked_multi_line_chart(
    handle: MultiLineChartHandle,
    link: ChartLinkHandle,
) -> impl ElementBuilder {
    linked_multi_line_chart_with_bindings(handle, link, crate::ChartInputBindings::default())
}

pub fn linked_multi_line_chart_with_bindings(
    handle: MultiLineChartHandle,
    link: ChartLinkHandle,
    bindings: crate::ChartInputBindings,
) -> impl ElementBuilder {
    crate::xy_stack::linked_x_chart(handle.0, link, bindings)
}
