---
type: uml.Sequence
title: Nested Exchange
---
# Nested Exchange

## Lifelines
- [A](./a.md) as a
- [B](./b.md) as b
- [C](./c.md) as c
- [D](./d.md) as d

## Messages
- a calls b: `start()`
- b calls c: `work()`
- c replies b: `done`
- b replies a: `ok`
- a sends b: `notify()`
- a creates d
- alt
  - when `ready`
    - b calls d: `init()`
    - opt
      - when `extra`
        - d replies b: `ack`
  - else
    - a calls b: `retry()`
- a destroys d
