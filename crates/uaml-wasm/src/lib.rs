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

/// Rebuild every `<dir>/index.md` from the bundle's package forest, leaving
/// concept/diagram docs untouched.
pub fn reindex_bundle_core(bundle: &[(String, String)]) -> Vec<(String, String)> {
    uaml::index_md::reindex_bundle(bundle)
}

/// Result of solving one diagram: absolute rects + any layout diagnostics.
/// Tsify emits its TypeScript type; under `wasm` it crosses the boundary as a
/// plain JS object.
#[derive(Debug, serde::Serialize, serde::Deserialize, tsify_next::Tsify)]
#[tsify(into_wasm_abi)]
pub struct SolveResult {
    pub solved: uaml::solve::Solved,
    pub diagnostics: Vec<uaml::diagnostic::Diagnostic>,
}

/// Build the model from `bundle`, pick the diagram whose `key == diagram_key`,
/// and solve it with the caller-supplied `sizes` + `cfg`. Errors if no diagram
/// matches the key (a caller bug, distinct from in-diagram graceful degradation).
pub fn solve_bundle(
    bundle: &[(String, String)],
    diagram_key: &str,
    sizes: uaml::solve::SizeMap,
    cfg: uaml::solve::SolveConfig,
) -> Result<SolveResult, String> {
    let model = uaml::parse::build_model(bundle);
    let diagram = model
        .diagrams
        .iter()
        .find(|d| d.key == diagram_key)
        .ok_or_else(|| format!("no diagram with key '{diagram_key}'"))?;
    let (solved, diagnostics) = uaml::solve::solve_diagram(diagram, &sizes, &cfg);
    Ok(SolveResult { solved, diagnostics })
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
/// untouched. `Concept.extra` (frontmatter) serializes as a plain JS object —
/// `serialize_maps_as_objects` matches its JSON semantics and the TS
/// `Record<string, FmValue>` type, not a `Map`.
#[wasm_bindgen]
pub fn build_bundle(bundle: JsValue) -> Result<JsValue, JsValue> {
    use serde::Serialize;
    let b: Vec<(String, String)> = serde_wasm_bindgen::from_value(bundle)?;
    let out = uaml::okf::build_bundle(&b);
    let ser = serde_wasm_bindgen::Serializer::new().serialize_maps_as_objects(true);
    Ok(out.serialize(&ser)?)
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

/// `bundle`: a `[path, markdown][]`. Returns the bundle with every
/// `<dir>/index.md` regenerated from the package forest.
#[wasm_bindgen]
pub fn reindex(bundle: JsValue) -> Result<JsValue, JsValue> {
    let b: Vec<(String, String)> = serde_wasm_bindgen::from_value(bundle)?;
    Ok(serde_wasm_bindgen::to_value(&reindex_bundle_core(&b))?)
}

/// Split a multi-document bundle string into `[path, markdown][]`.
#[wasm_bindgen]
pub fn split_bundle(text: &str) -> Result<JsValue, JsValue> {
    Ok(serde_wasm_bindgen::to_value(&uaml::parse::split_bundle(text))?)
}

/// `bundle`: `[path, markdown][]`; `diagram_key`: which diagram to solve;
/// `sizes`: `Record<string, {w, h}>`; `cfg`: `SolveConfig | null | undefined`.
/// Returns `{ solved, diagnostics }`.
#[wasm_bindgen]
pub fn solve(
    bundle: JsValue,
    diagram_key: String,
    sizes: JsValue,
    cfg: JsValue,
) -> Result<SolveResult, JsValue> {
    let b: Vec<(String, String)> = serde_wasm_bindgen::from_value(bundle)?;
    let sizes: uaml::solve::SizeMap = serde_wasm_bindgen::from_value(sizes)?;
    let cfg: uaml::solve::SolveConfig = if cfg.is_null() || cfg.is_undefined() {
        uaml::solve::SolveConfig::default()
    } else {
        serde_wasm_bindgen::from_value(cfg)?
    };
    solve_bundle(&b, &diagram_key, sizes, cfg).map_err(|e| JsValue::from_str(&e))
}
