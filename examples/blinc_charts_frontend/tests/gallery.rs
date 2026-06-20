use std::collections::BTreeSet;

use blinc_charts_frontend::gallery::{
    build_gallery_ui, sample_inventory, validate_sample_models, ChartFamily,
};

#[test]
fn sample_inventory_covers_required_chart_families() {
    let families: BTreeSet<_> = sample_inventory()
        .into_iter()
        .map(|sample| sample.family)
        .collect();

    for required in [
        ChartFamily::LinkedLine,
        ChartFamily::LinkedArea,
        ChartFamily::LinkedBar,
        ChartFamily::Heatmap,
        ChartFamily::Candlestick,
        ChartFamily::Histogram,
        ChartFamily::Statistics,
        ChartFamily::Gauge,
        ChartFamily::Funnel,
        ChartFamily::Polar,
    ] {
        assert!(
            families.contains(&required),
            "missing required chart family: {required:?}"
        );
    }
}

#[test]
fn sample_models_construct_successfully() {
    let report = validate_sample_models().expect("sample chart constructors should accept data");

    assert_eq!(report.linked_charts, 3);
    assert!(report.total_samples >= 10);
    assert!(report.total_points >= 300);
}

#[test]
fn sample_inventory_includes_interaction_metadata() {
    for sample in sample_inventory() {
        assert!(
            !sample.summary.is_empty(),
            "missing summary for {:?}",
            sample.family
        );
        assert!(
            !sample.interactions.is_empty(),
            "missing interactions for {:?}",
            sample.family
        );
    }
}

#[test]
fn linked_samples_document_shared_interactions() {
    let combined = sample_inventory()
        .into_iter()
        .filter(|sample| {
            matches!(
                sample.family,
                ChartFamily::LinkedLine | ChartFamily::LinkedArea | ChartFamily::LinkedBar
            )
        })
        .flat_map(|sample| sample.interactions.iter().copied())
        .collect::<Vec<_>>()
        .join(" ");

    for keyword in ["hover", "zoom", "pan", "brush"] {
        assert!(
            combined.contains(keyword),
            "linked samples should mention {keyword}"
        );
    }
}

#[test]
fn gallery_ui_builds_without_browser_runtime() {
    let _ui: blinc_layout::div::Div = build_gallery_ui().expect("gallery UI should build on host");
}
