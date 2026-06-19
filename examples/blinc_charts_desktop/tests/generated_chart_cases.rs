use std::collections::BTreeSet;

use blinc_charts::prelude::*;
use blinc_charts_desktop::gallery::{
    coverage_matrix, interaction_demo_inventory, sample_inventory, ChartFamily, CoverageCase,
};
use blinc_core::{Color, Point};
use blinc_layout::prelude::*;

#[test]
fn coverage_matrix_has_one_generated_build_for_every_case() {
    let cases = coverage_matrix();
    let generated: BTreeSet<_> = cases
        .iter()
        .map(|case| (case.family, case.variant, case.interaction))
        .collect();
    let expected_len: usize = sample_inventory()
        .iter()
        .map(|sample| sample.variants.len() * interaction_demo_inventory(sample.family).len())
        .sum();
    let expected_families: BTreeSet<_> = sample_inventory()
        .into_iter()
        .map(|sample| sample.family)
        .collect();
    let generated_families: BTreeSet<_> = generated.iter().map(|case| case.0).collect();

    assert_eq!(cases.len(), expected_len);
    assert_eq!(
        generated.len(),
        cases.len(),
        "coverage matrix has duplicates"
    );
    assert_eq!(generated_families, expected_families);
}

#[test]
fn generated_chart_cases_build_public_blinc_elements() {
    for case in coverage_matrix() {
        let _ui: blinc_layout::div::Div =
            build_generated_case(&case).unwrap_or_else(|err| panic!("{case:?} failed: {err}"));
    }
}

fn build_generated_case(case: &CoverageCase) -> anyhow::Result<blinc_layout::div::Div> {
    match case.family {
        ChartFamily::LinkedLine => linked_line(case.variant, case.interaction),
        ChartFamily::LinkedArea => linked_area(case.variant, case.interaction),
        ChartFamily::LinkedBar => linked_bar(case.variant, case.interaction),
        ChartFamily::MultiLine => multi_line(case.variant, case.interaction),
        ChartFamily::StackedArea => stacked_area(case.variant, case.interaction),
        ChartFamily::Scatter => scatter(case.variant, case.interaction),
        ChartFamily::Candlestick => candlestick(case.variant, case.interaction),
        ChartFamily::Histogram => histogram(case.variant, case.interaction),
        ChartFamily::Statistics => statistics(case.variant, case.interaction),
        ChartFamily::Heatmap => heatmap(case.variant),
        ChartFamily::Contour => contour(case.variant),
        ChartFamily::DensityMap => density_map(case.variant),
        ChartFamily::Gauge => gauge(case.variant),
        ChartFamily::Funnel => funnel(case.variant),
        ChartFamily::Polar => polar(case.variant),
        ChartFamily::Geo => geo(case.variant),
        ChartFamily::Hierarchy => hierarchy(case.variant),
        ChartFamily::Network => network(case.variant),
    }
}

fn chart_surface(chart: impl ElementBuilder + 'static) -> blinc_layout::div::Div {
    div().w(640.0).h(320.0).child(chart)
}

fn interaction_bindings(interaction: &str) -> ChartInputBindings {
    match interaction {
        "Shift+drag X brush" => ChartInputBindings {
            scroll_zoom: false,
            brush_drag: DragBinding {
                required: ModifiersReq::shift(),
                action: DragAction::BrushX,
            },
            ..ChartInputBindings::default()
        },
        "Drag-only brush binding" => ChartInputBindings {
            brush_drag: DragBinding {
                required: ModifiersReq::none(),
                action: DragAction::BrushX,
            },
            pan_drag: DragBinding::none(),
            scroll_zoom: false,
        },
        _ => ChartInputBindings {
            scroll_zoom: true,
            ..ChartInputBindings::default()
        },
    }
}

fn is_linked_sync(interaction: &str) -> bool {
    interaction == "Linked domain sync"
}

fn xs() -> Vec<f32> {
    (0..48).map(|idx| idx as f32).collect()
}

fn series(offset: f32) -> anyhow::Result<TimeSeriesF32> {
    let x = xs();
    let y = x
        .iter()
        .map(|v| offset + (*v * 0.2).sin() * 8.0 + *v * 0.1)
        .collect();
    Ok(TimeSeriesF32::new(x, y)?)
}

fn linked_line(variant: &str, interaction: &str) -> anyhow::Result<blinc_layout::div::Div> {
    let mut model = LineChartModel::new(series(30.0)?);
    model.style.line = Color::rgba(0.1, 0.7, 0.9, 1.0);
    match variant {
        "Interaction speed" => model.style.scroll_zoom_factor = 0.04,
        "Pinch floor" => model.style.pinch_zoom_min = 0.01,
        "Budget cap" => model.set_downsample_max_points(128),
        _ => {}
    }
    Ok(if is_linked_sync(interaction) {
        chart_surface(linked_line_chart_with_bindings(
            LineChartHandle::new(model),
            chart_link(0.0, 47.0),
            interaction_bindings(interaction),
        ))
    } else {
        chart_surface(line_chart_with_bindings(
            LineChartHandle::new(model),
            interaction_bindings(interaction),
        ))
    })
}

fn linked_area(variant: &str, interaction: &str) -> anyhow::Result<blinc_layout::div::Div> {
    let mut model = AreaChartModel::new(series(20.0)?);
    model.style.area = Color::rgba(0.9, 0.5, 0.2, 0.35);
    match variant {
        "Interaction speed" => model.style.scroll_zoom_factor = 0.04,
        "Pinch floor" => model.style.pinch_zoom_min = 0.01,
        "Budget cap" => model.set_downsample_max_points(128),
        _ => {}
    }
    Ok(if is_linked_sync(interaction) {
        chart_surface(linked_area_chart_with_bindings(
            AreaChartHandle::new(model),
            chart_link(0.0, 47.0),
            interaction_bindings(interaction),
        ))
    } else {
        chart_surface(area_chart_with_bindings(
            AreaChartHandle::new(model),
            interaction_bindings(interaction),
        ))
    })
}

fn linked_bar(variant: &str, interaction: &str) -> anyhow::Result<blinc_layout::div::Div> {
    let mut model = BarChartModel::new(vec![series(8.0)?, series(12.0)?])?;
    match variant {
        "Grouped bars" => model.style.stacked = false,
        "Stacked bars" => model.style.stacked = true,
        _ => {}
    }
    Ok(if is_linked_sync(interaction) {
        chart_surface(linked_bar_chart_with_bindings(
            BarChartHandle::new(model),
            chart_link(0.0, 47.0),
            interaction_bindings(interaction),
        ))
    } else {
        chart_surface(bar_chart_with_bindings(
            BarChartHandle::new(model),
            interaction_bindings(interaction),
        ))
    })
}

fn multi_line(variant: &str, interaction: &str) -> anyhow::Result<blinc_layout::div::Div> {
    let mut model = MultiLineChartModel::new(vec![series(10.0)?, series(18.0)?, series(26.0)?])?;
    match variant {
        "Interaction speed" => model.style.scroll_zoom_factor = 0.04,
        "Pinch floor" => model.style.pinch_zoom_min = 0.01,
        "Budget cap" => model.style.max_points_per_series = 256,
        _ => {}
    }
    Ok(if is_linked_sync(interaction) {
        chart_surface(linked_multi_line_chart_with_bindings(
            MultiLineChartHandle::new(model),
            chart_link(0.0, 47.0),
            interaction_bindings(interaction),
        ))
    } else {
        chart_surface(multi_line_chart_with_bindings(
            MultiLineChartHandle::new(model),
            interaction_bindings(interaction),
        ))
    })
}

fn stacked_area(variant: &str, interaction: &str) -> anyhow::Result<blinc_layout::div::Div> {
    let mut model = StackedAreaChartModel::new(vec![series(4.0)?, series(6.0)?, series(8.0)?])?;
    match variant {
        "Stacked" => model.style.mode = StackedAreaMode::Stacked,
        "Streamgraph" => model.style.mode = StackedAreaMode::Streamgraph,
        _ => {}
    }
    Ok(if is_linked_sync(interaction) {
        chart_surface(linked_stacked_area_chart_with_bindings(
            StackedAreaChartHandle::new(model),
            chart_link(0.0, 47.0),
            interaction_bindings(interaction),
        ))
    } else {
        chart_surface(stacked_area_chart_with_bindings(
            StackedAreaChartHandle::new(model),
            interaction_bindings(interaction),
        ))
    })
}

fn scatter(variant: &str, interaction: &str) -> anyhow::Result<blinc_layout::div::Div> {
    let mut model = ScatterChartModel::new(series(14.0)?);
    match variant {
        "Interaction speed" => model.style.scroll_zoom_factor = 0.04,
        "Pinch floor" => model.style.pinch_zoom_min = 0.01,
        "Budget cap" => model.set_max_points(500),
        _ => {}
    }
    Ok(if is_linked_sync(interaction) {
        chart_surface(linked_scatter_chart_with_bindings(
            ScatterChartHandle::new(model),
            chart_link(0.0, 47.0),
            interaction_bindings(interaction),
        ))
    } else {
        chart_surface(scatter_chart_with_bindings(
            ScatterChartHandle::new(model),
            interaction_bindings(interaction),
        ))
    })
}

fn candlestick(variant: &str, interaction: &str) -> anyhow::Result<blinc_layout::div::Div> {
    let candles = xs()
        .into_iter()
        .map(|x| Candle {
            x,
            open: 20.0 + x * 0.1,
            high: 24.0 + x * 0.1,
            low: 18.0 + x * 0.1,
            close: 21.0 + (x * 0.2).sin(),
        })
        .collect();
    let mut model = CandlestickChartModel::new(CandleSeries::new(candles)?);
    match variant {
        "Interaction speed" => model.style.scroll_zoom_factor = 0.04,
        "Pinch floor" => model.style.pinch_zoom_min = 0.01,
        "Budget cap" => model.style.max_candles = 256,
        _ => {}
    }
    Ok(if is_linked_sync(interaction) {
        chart_surface(linked_candlestick_chart_with_bindings(
            CandlestickChartHandle::new(model),
            chart_link(0.0, 47.0),
            interaction_bindings(interaction),
        ))
    } else {
        chart_surface(candlestick_chart_with_bindings(
            CandlestickChartHandle::new(model),
            interaction_bindings(interaction),
        ))
    })
}

fn histogram(variant: &str, interaction: &str) -> anyhow::Result<blinc_layout::div::Div> {
    let values = (0..120)
        .map(|idx| ((idx as f32) * 0.17).sin() * 20.0)
        .collect();
    let mut model = HistogramChartModel::new(values)?;
    match variant {
        "Interaction speed" => model.style.scroll_zoom_factor = 0.04,
        "Pinch floor" => model.style.pinch_zoom_min = 0.01,
        "Budget cap" => model.style.bins = 24,
        _ => {}
    }
    Ok(chart_surface(histogram_chart_with_bindings(
        HistogramChartHandle::new(model),
        interaction_bindings(interaction),
    )))
}

fn statistics(variant: &str, interaction: &str) -> anyhow::Result<blinc_layout::div::Div> {
    let groups = (0..4)
        .map(|g| {
            (0..32)
                .map(|i| g as f32 * 8.0 + (i as f32 * 0.3).sin())
                .collect()
        })
        .collect();
    let mut model = StatisticsChartModel::new(groups)?;
    match variant {
        "Boxplot" => model.style.mode = StatisticsMode::Boxplot,
        "Violin" => model.style.mode = StatisticsMode::Violin,
        "Error band" => model.style.mode = StatisticsMode::ErrorBand,
        _ => {}
    }
    Ok(chart_surface(statistics_chart_with_bindings(
        StatisticsChartHandle::new(model),
        interaction_bindings(interaction),
    )))
}

fn grid_values(w: usize, h: usize) -> Vec<f32> {
    (0..h)
        .flat_map(|y| (0..w).map(move |x| (x as f32 * 0.3).sin() + (y as f32 * 0.2).cos()))
        .collect()
}

fn heatmap(variant: &str) -> anyhow::Result<blinc_layout::div::Div> {
    let mut model = HeatmapChartModel::new(12, 8, grid_values(12, 8))?;
    match variant {
        "Higher resolution" => {
            model.style.max_cells_x = 128;
            model.style.max_cells_y = 64;
        }
        "Lower resolution" => {
            model.style.max_cells_x = 8;
            model.style.max_cells_y = 6;
        }
        _ => {}
    }
    Ok(chart_surface(heatmap_chart(HeatmapChartHandle::new(model))))
}

fn contour(variant: &str) -> anyhow::Result<blinc_layout::div::Div> {
    let mut model = ContourChartModel::new(16, 12, grid_values(16, 12))?;
    match variant {
        "Higher resolution" => model.style.max_segments = 4_000,
        "Lower resolution" => model.style.max_segments = 250,
        _ => {}
    }
    model.style.scroll_zoom_factor = 0.02;
    Ok(chart_surface(contour_chart(ContourChartHandle::new(model))))
}

fn density_map(variant: &str) -> anyhow::Result<blinc_layout::div::Div> {
    let points = (0..160)
        .map(|idx| Point {
            x: (idx % 20) as f32,
            y: (idx / 20) as f32 + (idx as f32 * 0.1).sin(),
        })
        .collect();
    let mut model = DensityMapChartModel::new(points)?;
    match variant {
        "Higher resolution" => {
            model.style.max_cells_x = 64;
            model.style.max_cells_y = 48;
        }
        "Lower resolution" => {
            model.style.max_cells_x = 16;
            model.style.max_cells_y = 12;
        }
        _ => {}
    }
    model.style.scroll_zoom_factor = 0.02;
    Ok(chart_surface(density_map_chart(
        DensityMapChartHandle::new(model),
    )))
}

fn gauge(variant: &str) -> anyhow::Result<blinc_layout::div::Div> {
    let mut model = GaugeChartModel::new(0.0, 100.0, 72.0)?;
    match variant {
        "Immediate value" => model.set_value(86.0),
        "Transitioned value" => model.set_value_transition(86.0, 0.25),
        _ => {}
    }
    Ok(chart_surface(gauge_chart(GaugeChartHandle::new(model))))
}

fn funnel(variant: &str) -> anyhow::Result<blinc_layout::div::Div> {
    let stages = match variant {
        "Budget cap" => vec![
            ("Visitors".to_string(), 1200.0),
            ("Paid".to_string(), 180.0),
        ],
        _ => vec![
            ("Visitors".to_string(), 1200.0),
            ("Trials".to_string(), 420.0),
            ("Paid".to_string(), 180.0),
        ],
    };
    Ok(chart_surface(funnel_chart(FunnelChartHandle::new(
        FunnelChartModel::new(stages)?,
    ))))
}

fn polar(variant: &str) -> anyhow::Result<blinc_layout::div::Div> {
    let dimensions = vec![
        "Speed".into(),
        "Quality".into(),
        "Cost".into(),
        "Risk".into(),
    ];
    let series = vec![vec![0.8, 0.6, 0.4, 0.3], vec![0.5, 0.9, 0.7, 0.6]];
    let mut model = PolarChartModel::new_radar(dimensions, series)?;
    match variant {
        "Radar" => model.mode = PolarChartMode::Radar,
        "Polar" => model.mode = PolarChartMode::Polar,
        "Parallel" => model.mode = PolarChartMode::Parallel,
        _ => {}
    }
    Ok(chart_surface(polar_chart(PolarChartHandle::new(model))))
}

fn geo(variant: &str) -> anyhow::Result<blinc_layout::div::Div> {
    let shapes = vec![vec![
        Point { x: 0.0, y: 0.0 },
        Point { x: 10.0, y: 1.0 },
        Point { x: 8.0, y: 7.0 },
        Point { x: 1.0, y: 6.0 },
    ]];
    let mut model = GeoChartModel::new(shapes)?;
    match variant {
        "Interaction speed" => model.style.scroll_zoom_factor = 0.04,
        "Pinch floor" => model.style.pinch_zoom_min = 0.01,
        "Budget cap" => model.style.max_points = 2_000,
        _ => {}
    }
    Ok(chart_surface(geo_chart(GeoChartHandle::new(model))))
}

fn hierarchy(variant: &str) -> anyhow::Result<blinc_layout::div::Div> {
    let root = HierarchyNode::node(
        "Root",
        vec![
            HierarchyNode::leaf("Alpha", 4.0),
            HierarchyNode::leaf("Beta", 2.0),
            HierarchyNode::leaf("Gamma", 3.0),
        ],
    );
    let mut model = HierarchyChartModel::new(root)?;
    match variant {
        "Treemap" => model.style.mode = HierarchyMode::Treemap,
        "Sunburst" => model.style.mode = HierarchyMode::Sunburst,
        "Packing" => model.style.mode = HierarchyMode::Packing,
        _ => {}
    }
    Ok(chart_surface(hierarchy_chart(HierarchyChartHandle::new(
        model,
    ))))
}

fn network(variant: &str) -> anyhow::Result<blinc_layout::div::Div> {
    let model = match variant {
        "Graph" => NetworkChartModel::new_graph(
            vec!["Source".into(), "Mid".into(), "Sink".into()],
            vec![(0, 1), (1, 2)],
        )?,
        "Chord" => NetworkChartModel::new_chord(
            vec!["A".into(), "B".into(), "C".into()],
            vec![
                vec![0.0, 3.0, 1.0],
                vec![2.0, 0.0, 4.0],
                vec![1.0, 2.0, 0.0],
            ],
        )?,
        _ => NetworkChartModel::new_sankey(
            vec!["Source".into(), "Mid".into(), "Sink".into()],
            vec![(0, 1, 8.0), (1, 2, 5.0)],
        )?,
    };
    Ok(chart_surface(network_chart(NetworkChartHandle::new(model))))
}
