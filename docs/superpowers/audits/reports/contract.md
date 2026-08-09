# Changed files

- `docs/waml/documentation-contract.md`
- `docs/waml/waml-feature-gaps.md`
- `docs/superpowers/audits/reports/contract.md`

# Scenario identifiers

None.

# Evidence

The integrated documentation contract has four clean gates:

- `rtk cargo run -p waml-cli -- check docs/waml`
- `rtk cargo run -p waml-cli -- fmt --check docs/waml`
- `rtk cargo run -p waml-cli -- index docs/waml --check`
- `rtk node scripts/check-waml-doc-contract.mjs docs/waml`

The generated root index links the documentation contract, feature-gap ledger,
goal tree, product-use-case tree, architecture tree, and user document.

# Verification gaps

None in the contract or feature-gap ledger. The integrated goal tree records
53 inventory verification gaps and 32 stable sequence-language verification
gaps.

# Open discrepancies

None in the contract or feature-gap ledger.

# Feature gaps

- `FG-001` through `FG-010` are recorded in
  `docs/waml/waml-feature-gaps.md`.
- `FG-010` records scenario-to-product-use-case-to-test-and-evidence
  traceability. The product-use-case model supplies all 144 goal scenario
  links.
- Specialized actor, use-case, and system-boundary rendering is separate user
  work. It is not a documentation-contract feature gap.
