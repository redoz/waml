# Tsify spike findings (2026-07-15)

## SPIKE-A-RESULT (Open Risk #1: #[serde(default)] optionality)
- `#[serde(default)]` `v: u32` with no tsify attr generated as: `v?: number;`
- `Option<T>` + skip, no tsify attr, generated as: `opt_plain?: string;`
- `#[tsify(optional)]` on `Option<T>` + skip: supported — compiles cleanly, generated as: `opt_marked?: string;` (identical to the unmarked `opt_plain` field)
- DECISION: omit optional token; tsify renders fields with `#[serde(default, ...)]`/`Option<T>` as `field?: T` already

## SPIKE-B-RESULT (Open Risk #2: map field rendering through tsify into_wasm_abi)
- `out.m instanceof Map`: true
- object access `out.m.owner`: undefined
- DECISION for build_bundle (Task 7): "tsify emits a Map → KEEP build_bundle returning JsValue with serialize_maps_as_objects; add serialize_btreemap_as_object-style override only if flipping later"
