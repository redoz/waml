---
type: uml.SequenceDiagram
title: Nested Exchange
---
# Nested Exchange

## Lifelines
- [A](./a.md) as a
- [B](./b.md) as b
- [C](./c.md) as c
- [D](./d.md) as d

## Messages
- a calls b `start()`
- b calls c `work()`
- c returns `done` to b
- b returns `ok` to a
- a signals b `notify()`
- a creates d
- alt
  - when `ready`
    - b calls d `init()`
    - opt
      - when `extra`
        - d returns `ack` to b
  - else
    - a calls b `retry()`
- a destroys d
