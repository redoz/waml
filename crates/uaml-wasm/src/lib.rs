//! Thin WASM bindings over the UAML core. Each `#[wasm_bindgen]` entry point is a
//! serde-wasm-bindgen shell around a pure `*_json`/`*_bundle` core that is unit-tested
//! natively (see `tests/native.rs`).
use wasm_bindgen::prelude::*;

// ── Pure, natively-testable cores ────────────────────────────────────────────

pub fn build_model_json(bundle: &[(String, String)]) -> String {
    serde_json::to_string(&uaml::parse::build_model(bundle)).unwrap()
}

pub fn validate_json(bundle: &[(String, String)]) -> String {
    serde_json::to_string(&uaml::validate::validate(bundle)).unwrap()
}

// ── wasm-bindgen surface (structured JS values via serde-wasm-bindgen) ────────

#[wasm_bindgen]
pub fn init_panic_hook() {
    console_error_panic_hook::set_once();
}

/// `bundle`: a `[path, markdown][]` (array of pairs). Returns the resolved `Model`.
#[wasm_bindgen]
pub fn build_model(bundle: JsValue) -> Result<JsValue, JsValue> {
    let b: Vec<(String, String)> = serde_wasm_bindgen::from_value(bundle)?;
    let model = uaml::parse::build_model(&b);
    Ok(serde_wasm_bindgen::to_value(&model)?)
}

/// `bundle`: a `[path, markdown][]`. Returns a `Diagnostic[]`.
#[wasm_bindgen]
pub fn validate(bundle: JsValue) -> Result<JsValue, JsValue> {
    let b: Vec<(String, String)> = serde_wasm_bindgen::from_value(bundle)?;
    let diags = uaml::validate::validate(&b);
    Ok(serde_wasm_bindgen::to_value(&diags)?)
}
