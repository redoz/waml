---
type: uml.Sequence
title: Broken sequence
---
# Broken sequence

## Lifelines
- [Target](./target.md) as target
- missing-link

## Messages
- alt
   - when `bad-indent`
  - when `ok`
    - target calls nowhere
    - opt
      - else
        - target sends target
- par

## Relationships
- depends [Target](./target.md)
