//! Compiles `ui/settings.slint` (fluent style) into generated Rust bindings
//! under OUT_DIR. The .slint files under `ui/` are the single source of
//! truth for the settings window.

fn main() {
    let config = slint_build::CompilerConfiguration::new().with_style("fluent".to_string());
    slint_build::compile_with_config("ui/settings.slint", config)
        .expect("failed to compile ui/settings.slint");
}
