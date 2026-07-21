//! The generic single-active popup mechanic. `PopupRoot` (an authority widget)
//! hosts at most one active ephemeral surface and runs universal light-dismiss;
//! `MenuPopup` (linear card) and `RadialPopup` (wedge) are the two surface kinds,
//! both driven through the `Popup` trait and both embedding the shared
//! `MarkingCore`. See `docs/superpowers/specs/2026-07-21-generic-popup-mechanic-design.md`.

pub mod base;
pub mod marking;
// Filled by later tasks:
// pub mod radial;    // Task 3
// pub mod menu;      // Task 4
// pub mod presenter; // Task 5
// pub mod root;      // Task 6
