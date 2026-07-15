# Tsify spike findings (2026-07-15)

## SPIKE-A-RESULT (Open Risk #1: #[serde(default)] optionality)
- `#[serde(default)]` `v: u32` with no tsify attr generated as: `v?: number;`
- `Option<T>` + skip, no tsify attr, generated as: `opt_plain?: string;`
- `#[tsify(optional)]` on `Option<T>` + skip: supported — compiles cleanly, generated as: `opt_marked?: string;` (identical to the unmarked `opt_plain` field)
- DECISION: omit optional token; tsify renders fields with `#[serde(default, ...)]`/`Option<T>` as `field?: T` already
