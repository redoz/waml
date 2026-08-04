---
title: Reading View
---

# Reading View

A concept opens as prose to read. Markdown punctuation is *rendered*, not
shown, and `inline code` keeps its own face.

## Nesting

- outer bullet
  - inner bullet
    - deepest bullet
- second outer

1. first ordered
2. second ordered

> A quote wraps the paragraph it contains, and its angle bracket is
> punctuation the reader never sees.

```rust
fn main() {
    println!("fenced code keeps its content");
}
```

| Column | Meaning |
| ------ | ------- |
| one    | first   |
| two    | second  |
