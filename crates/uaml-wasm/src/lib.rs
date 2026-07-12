//! Thin WASM bindings over the UAML core. Each `#[wasm_bindgen]` entry point is a
//! serde-wasm-bindgen shell around a pure `*_json`/`*_bundle` core that is unit-tested
//! natively (see `tests/native.rs`).
use wasm_bindgen::prelude::*;

// ── Pure, natively-testable cores ────────────────────────────────────────────

pub fn build_model_json(bundle: &[(String, String)]) -> String {
    serde_json::to_string(&uaml::parse::build_model(bundle)).unwrap()
}

/// Project each document to its OKF [`Concept`](uaml::okf::Concept), returning
/// the resolved [`Bundle`](uaml::okf::Bundle) as JSON. Additive to
/// [`build_model_json`]: domain-agnostic, lossless, and it does not touch the
/// UML `Model` shape.
pub fn build_bundle_json(bundle: &[(String, String)]) -> String {
    serde_json::to_string(&uaml::okf::build_bundle(bundle)).unwrap()
}

pub fn validate_json(bundle: &[(String, String)]) -> String {
    serde_json::to_string(&uaml::validate::validate(bundle)).unwrap()
}

/// Apply a JSON `OpDto[]` to a bundle, returning the edited bundle (or a
/// `op {index}: {reason}` error string).
pub fn apply_ops_bundle(
    bundle: &[(String, String)],
    ops_json: &str,
) -> Result<Vec<(String, String)>, String> {
    let dtos: Vec<uaml_ops_dto::OpDto> =
        serde_json::from_str(ops_json).map_err(|e| e.to_string())?;
    let ops = dtos_to_ops(dtos)?;
    uaml::ops::apply(bundle, &ops).map_err(|e| format!("op {}: {}", e.index, e.reason))
}

fn dtos_to_ops(dtos: Vec<uaml_ops_dto::OpDto>) -> Result<Vec<uaml::ops::Op>, String> {
    dtos.into_iter().map(|d| d.to_op()).collect()
}

/// Canonicalize each document (serialize IS fmt). Idempotent by construction.
pub fn fmt_bundle(bundle: &[(String, String)]) -> Vec<(String, String)> {
    bundle
        .iter()
        .map(|(p, t)| {
            (
                p.clone(),
                uaml::serialize::serialize_document(&uaml::parse::parse_document(t)),
            )
        })
        .collect()
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

/// `bundle`: a `[path, markdown][]`. Returns the resolved OKF `Bundle` (one
/// `Concept` per document). Additive to [`build_model`]; the UML surface is
/// untouched.
#[wasm_bindgen]
pub fn build_bundle(bundle: JsValue) -> Result<JsValue, JsValue> {
    let b: Vec<(String, String)> = serde_wasm_bindgen::from_value(bundle)?;
    let out = uaml::okf::build_bundle(&b);
    Ok(serde_wasm_bindgen::to_value(&out)?)
}

/// `bundle`: a `[path, markdown][]`. Returns a `Diagnostic[]`.
#[wasm_bindgen]
pub fn validate(bundle: JsValue) -> Result<JsValue, JsValue> {
    let b: Vec<(String, String)> = serde_wasm_bindgen::from_value(bundle)?;
    let diags = uaml::validate::validate(&b);
    Ok(serde_wasm_bindgen::to_value(&diags)?)
}

/// `bundle`: a `[path, markdown][]`; `ops`: an `OpDto[]`. Returns the edited bundle.
#[wasm_bindgen]
pub fn apply_ops(bundle: JsValue, ops: JsValue) -> Result<JsValue, JsValue> {
    let b: Vec<(String, String)> = serde_wasm_bindgen::from_value(bundle)?;
    let dtos: Vec<uaml_ops_dto::OpDto> = serde_wasm_bindgen::from_value(ops)?;
    let parsed = dtos_to_ops(dtos).map_err(|e| JsValue::from_str(&e))?;
    let out = uaml::ops::apply(&b, &parsed)
        .map_err(|e| JsValue::from_str(&format!("op {}: {}", e.index, e.reason)))?;
    Ok(serde_wasm_bindgen::to_value(&out)?)
}

/// `bundle`: a `[path, markdown][]`. Returns the canonicalized bundle.
#[wasm_bindgen]
pub fn fmt(bundle: JsValue) -> Result<JsValue, JsValue> {
    let b: Vec<(String, String)> = serde_wasm_bindgen::from_value(bundle)?;
    Ok(serde_wasm_bindgen::to_value(&fmt_bundle(&b))?)
}

/// Split a multi-document bundle string into `[path, markdown][]`.
#[wasm_bindgen]
pub fn split_bundle(text: &str) -> Result<JsValue, JsValue> {
    Ok(serde_wasm_bindgen::to_value(&uaml::parse::split_bundle(text))?)
}
