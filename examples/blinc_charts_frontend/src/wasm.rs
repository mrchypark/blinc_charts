#![cfg(target_arch = "wasm32")]

use blinc_app::web::WebApp;
use blinc_app::windowed::WindowedContext;
use blinc_layout::div::Div;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    tracing_wasm::set_as_global_default_with_config(
        tracing_wasm::WASMLayerConfigBuilder::new()
            .set_max_level(tracing::Level::INFO)
            .build(),
    );

    wasm_bindgen_futures::spawn_local(async {
        let result = WebApp::run_with_setup(
            "blinc-canvas",
            |_| {
                web_sys::console::log_1(
                    &"blinc_charts_frontend: running without bundled fonts".into(),
                );
            },
            build_ui,
        )
        .await;

        if let Err(e) = result {
            web_sys::console::error_1(
                &format!("blinc_charts_frontend: WebApp::run failed: {e}").into(),
            );
        }
    });
}

fn build_ui(_ctx: &mut WindowedContext) -> Div {
    crate::gallery::build_gallery_ui().unwrap_or_else(|e| {
        use blinc_core::Color;
        use blinc_layout::prelude::*;

        div()
            .w_full()
            .h_full()
            .bg(Color::rgba(0.06, 0.07, 0.09, 1.0))
            .items_center()
            .justify_center()
            .child(
                text(format!(
                    "blinc_charts_frontend failed to build gallery: {e}"
                ))
                .size(16.0)
                .color(Color::rgba(0.95, 0.78, 0.62, 1.0)),
            )
    })
}
