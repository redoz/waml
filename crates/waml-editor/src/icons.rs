//! The tree/doc-tab kind glyph set, hand-authored as SDF shaders (replacing the
//! blurry `resources/icons/*.svg` + `DrawSvg` path). One shader per kind, so
//! each `pixel: fn` stays small, reads alone, and hot-reloads independently --
//! same one-shader-per-primitive idiom as `frame.rs`.
//!
//! Material: the Atlas "HUD" language -- single accent tint (`atlas.accent`),
//! hollow interiors (low-alpha fill + thin stroke). Sharp corners use
//! `sdf.rect`/paths, `sdf.box` only where a real corner radius is wanted; never
//! `sdf.box(.., 0.0)` (degenerates + floods). Geometry is authored in the
//! shader's local `rect_size` (the ~14px display size), so stroke widths are
//! chosen for that size instead of scaled down from a 24-unit SVG viewBox.
//!
//! Silhouettes here are a first pass, tuned live in the `icon_harness` bin.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas

    // Class: rounded body + a header divider line (the UML class compartment).
    mod.draw.IconClass = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.085
            let m = s * 0.16
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.box(m, m, s - 2.0 * m, s - 2.0 * m, s * 0.09)
            sdf.stroke(self.color, w)
            // Header divider: a full-width stroke straight through the body, at
            // the outline weight so it survives 14px (a thin fill bar vanished).
            sdf.move_to(m, s * 0.42)
            sdf.line_to(s - m, s * 0.42)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Interface: the same rounded-square body as the class card, minus the
    // header divider -- keeps the classifier family visually related.
    mod.draw.IconInterface = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.085
            let m = s * 0.16
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.box(m, m, s - 2.0 * m, s - 2.0 * m, s * 0.09)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Enum: three squares (top-left, top-right, bottom-left).
    mod.draw.IconEnum = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.075
            let m = s * 0.15
            let g = s * 0.17
            let d = (s - 2.0 * m - g) * 0.5
            let r = s * 0.05
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.box(m, m, d, d, r)
            sdf.stroke(self.color, w)
            sdf.box(m + d + g, m, d, d, r)
            sdf.stroke(self.color, w)
            sdf.box(m, m + d + g, d, d, r)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // DataType: a pointy-top hexagon outline.
    mod.draw.IconDataType = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.085
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.5, s * 0.15)
            sdf.line_to(s * 0.80, s * 0.32)
            sdf.line_to(s * 0.80, s * 0.68)
            sdf.line_to(s * 0.5, s * 0.85)
            sdf.line_to(s * 0.20, s * 0.68)
            sdf.line_to(s * 0.20, s * 0.32)
            sdf.close_path()
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Package: the same hexagon read as a cube, with three interior seams.
    mod.draw.IconPackage = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.085
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.5, s * 0.10)
            sdf.line_to(s * 0.85, s * 0.30)
            sdf.line_to(s * 0.85, s * 0.70)
            sdf.line_to(s * 0.5, s * 0.90)
            sdf.line_to(s * 0.15, s * 0.70)
            sdf.line_to(s * 0.15, s * 0.30)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.15, s * 0.30)
            sdf.line_to(s * 0.5, s * 0.5)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.85, s * 0.30)
            sdf.line_to(s * 0.5, s * 0.5)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.5, s * 0.5)
            sdf.line_to(s * 0.5, s * 0.90)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Diagram-view family: a sharp canvas frame + a minimal interior mark.
    // Diagram: two nodes joined by a link (node-graph) inside the frame.
    mod.draw.IconDiagram = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.07
            let m = s * 0.06
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.rect(m, m, s - 2.0 * m, s - 2.0 * m)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.40, s * 0.42)
            sdf.line_to(s * 0.60, s * 0.58)
            sdf.stroke(self.color, w)
            sdf.circle(s * 0.38, s * 0.42, s * 0.07)
            sdf.fill(self.color)
            sdf.circle(s * 0.62, s * 0.58, s * 0.07)
            sdf.fill(self.color)
            return sdf.result
        }
    }

    // Flow: a decision diamond inside the canvas frame -- activity/behavior.
    mod.draw.IconFlow = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.07
            let m = s * 0.06
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.rect(m, m, s - 2.0 * m, s - 2.0 * m)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.5, s * 0.30)
            sdf.line_to(s * 0.68, s * 0.5)
            sdf.line_to(s * 0.5, s * 0.70)
            sdf.line_to(s * 0.32, s * 0.5)
            sdf.close_path()
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Sequence: two stacked message bars inside the canvas frame -- exchange.
    mod.draw.IconSequence = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.07
            let m = s * 0.06
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.rect(m, m, s - 2.0 * m, s - 2.0 * m)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.32, s * 0.42)
            sdf.line_to(s * 0.62, s * 0.42)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.38, s * 0.58)
            sdf.line_to(s * 0.68, s * 0.58)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Note: a dog-eared page.
    mod.draw.IconNote = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.085
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.27, s * 0.16)
            sdf.line_to(s * 0.60, s * 0.16)
            sdf.line_to(s * 0.75, s * 0.31)
            sdf.line_to(s * 0.75, s * 0.84)
            sdf.line_to(s * 0.27, s * 0.84)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.60, s * 0.16)
            sdf.line_to(s * 0.60, s * 0.31)
            sdf.line_to(s * 0.75, s * 0.31)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Message: a speech bubble (rounded body + tail) with three text lines.
    // Not yet mapped to a kind -- authored ahead for later (e.g. comments/chat).
    mod.draw.IconMessage = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.075
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.box(s * 0.12, s * 0.14, s * 0.76, s * 0.54, s * 0.09)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.34, s * 0.68)
            sdf.line_to(s * 0.18, s * 0.86)
            sdf.line_to(s * 0.26, s * 0.68)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.26, s * 0.30)
            sdf.line_to(s * 0.55, s * 0.30)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.26, s * 0.42)
            sdf.line_to(s * 0.62, s * 0.42)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.26, s * 0.54)
            sdf.line_to(s * 0.48, s * 0.54)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Package+: the package cube with a plus badge (add-to-package action).
    mod.draw.IconPackagePlus = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.075
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.44, s * 0.12)
            sdf.line_to(s * 0.72, s * 0.28)
            sdf.line_to(s * 0.72, s * 0.58)
            sdf.line_to(s * 0.44, s * 0.74)
            sdf.line_to(s * 0.16, s * 0.58)
            sdf.line_to(s * 0.16, s * 0.28)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.16, s * 0.28)
            sdf.line_to(s * 0.44, s * 0.43)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.72, s * 0.28)
            sdf.line_to(s * 0.44, s * 0.43)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.44, s * 0.43)
            sdf.line_to(s * 0.44, s * 0.74)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.66, s * 0.78)
            sdf.line_to(s * 0.90, s * 0.78)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.78, s * 0.66)
            sdf.line_to(s * 0.78, s * 0.90)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Paintbrush (vertical): ferrule box + two bristle ticks + handle.
    mod.draw.IconPaintbrush = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.075
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.box(s * 0.25, s * 0.13, s * 0.50, s * 0.37, s * 0.06)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.42, s * 0.04)
            sdf.line_to(s * 0.42, s * 0.13)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.58, s * 0.04)
            sdf.line_to(s * 0.58, s * 0.13)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.25, s * 0.50)
            sdf.line_to(s * 0.30, s * 0.63)
            sdf.line_to(s * 0.44, s * 0.63)
            sdf.line_to(s * 0.44, s * 0.86)
            sdf.line_to(s * 0.56, s * 0.86)
            sdf.line_to(s * 0.56, s * 0.63)
            sdf.line_to(s * 0.70, s * 0.63)
            sdf.line_to(s * 0.75, s * 0.50)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Pin: pushpin head + needle.
    mod.draw.IconPin = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.075
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.5, s * 0.71)
            sdf.line_to(s * 0.5, s * 0.92)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.32, s * 0.29)
            sdf.line_to(s * 0.68, s * 0.29)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.34, s * 0.29)
            sdf.line_to(s * 0.40, s * 0.56)
            sdf.line_to(s * 0.5, s * 0.71)
            sdf.line_to(s * 0.60, s * 0.56)
            sdf.line_to(s * 0.66, s * 0.29)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Pin-off: pin body with a diagonal strike (unpinned).
    mod.draw.IconPinOff = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.075
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.5, s * 0.71)
            sdf.line_to(s * 0.5, s * 0.92)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.34, s * 0.29)
            sdf.line_to(s * 0.40, s * 0.56)
            sdf.line_to(s * 0.5, s * 0.71)
            sdf.line_to(s * 0.60, s * 0.56)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.32, s * 0.29)
            sdf.line_to(s * 0.68, s * 0.29)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.10, s * 0.10)
            sdf.line_to(s * 0.90, s * 0.90)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Share: open tray + up-and-out arrow.
    mod.draw.IconShare = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.075
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.17, s * 0.50)
            sdf.line_to(s * 0.17, s * 0.85)
            sdf.line_to(s * 0.83, s * 0.85)
            sdf.line_to(s * 0.83, s * 0.50)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.5, s * 0.10)
            sdf.line_to(s * 0.5, s * 0.62)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.33, s * 0.25)
            sdf.line_to(s * 0.5, s * 0.10)
            sdf.line_to(s * 0.67, s * 0.25)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Spline: two endpoint nodes joined by a curve (polyline approximation).
    mod.draw.IconSpline = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.075
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.21, s * 0.70)
            sdf.line_to(s * 0.28, s * 0.42)
            sdf.line_to(s * 0.42, s * 0.28)
            sdf.line_to(s * 0.70, s * 0.21)
            sdf.stroke(self.color, w)
            sdf.circle(s * 0.79, s * 0.21, s * 0.10)
            sdf.stroke(self.color, w)
            sdf.circle(s * 0.21, s * 0.79, s * 0.10)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Spline-pointer: a spline curve with a cursor pointer at the tail.
    mod.draw.IconSplinePointer = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.075
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.21, s * 0.70)
            sdf.line_to(s * 0.28, s * 0.42)
            sdf.line_to(s * 0.42, s * 0.28)
            sdf.line_to(s * 0.70, s * 0.21)
            sdf.stroke(self.color, w)
            sdf.circle(s * 0.79, s * 0.21, s * 0.10)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.50, s * 0.52)
            sdf.line_to(s * 0.58, s * 0.90)
            sdf.line_to(s * 0.66, s * 0.70)
            sdf.line_to(s * 0.86, s * 0.62)
            sdf.close_path()
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Square-minus: rounded square with a minus.
    mod.draw.IconSquareMinus = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.075
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.box(s * 0.13, s * 0.13, s * 0.74, s * 0.74, s * 0.09)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.33, s * 0.5)
            sdf.line_to(s * 0.67, s * 0.5)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Square-plus: rounded square with a plus.
    mod.draw.IconSquarePlus = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.075
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.box(s * 0.13, s * 0.13, s * 0.74, s * 0.74, s * 0.09)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.33, s * 0.5)
            sdf.line_to(s * 0.67, s * 0.5)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.5, s * 0.33)
            sdf.line_to(s * 0.5, s * 0.67)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Trash: lid, handle, and can body.
    mod.draw.IconTrash = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.075
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.13, s * 0.27)
            sdf.line_to(s * 0.87, s * 0.27)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.34, s * 0.27)
            sdf.line_to(s * 0.34, s * 0.15)
            sdf.line_to(s * 0.66, s * 0.15)
            sdf.line_to(s * 0.66, s * 0.27)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.21, s * 0.27)
            sdf.line_to(s * 0.24, s * 0.85)
            sdf.line_to(s * 0.76, s * 0.85)
            sdf.line_to(s * 0.79, s * 0.27)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // List collapse (down-up): three rows + chevrons pointing inward.
    mod.draw.IconListCollapse = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.075
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.13, s * 0.21)
            sdf.line_to(s * 0.46, s * 0.21)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.13, s * 0.50)
            sdf.line_to(s * 0.46, s * 0.50)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.13, s * 0.79)
            sdf.line_to(s * 0.46, s * 0.79)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.625, s * 0.21)
            sdf.line_to(s * 0.75, s * 0.33)
            sdf.line_to(s * 0.875, s * 0.21)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.625, s * 0.79)
            sdf.line_to(s * 0.75, s * 0.67)
            sdf.line_to(s * 0.875, s * 0.79)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // List expand (up-down): three rows + chevrons pointing outward.
    mod.draw.IconListExpand = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.075
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.13, s * 0.21)
            sdf.line_to(s * 0.46, s * 0.21)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.13, s * 0.50)
            sdf.line_to(s * 0.46, s * 0.50)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.13, s * 0.79)
            sdf.line_to(s * 0.46, s * 0.79)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.625, s * 0.33)
            sdf.line_to(s * 0.75, s * 0.21)
            sdf.line_to(s * 0.875, s * 0.33)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.625, s * 0.67)
            sdf.line_to(s * 0.75, s * 0.79)
            sdf.line_to(s * 0.875, s * 0.67)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Pencil: diagonal body tapering to a tip, with the ferrule band.
    mod.draw.IconPencil = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.075
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.72, s * 0.13)
            sdf.line_to(s * 0.88, s * 0.29)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.72, s * 0.13)
            sdf.line_to(s * 0.13, s * 0.72)
            sdf.line_to(s * 0.08, s * 0.92)
            sdf.line_to(s * 0.28, s * 0.87)
            sdf.line_to(s * 0.88, s * 0.29)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.60, s * 0.21)
            sdf.line_to(s * 0.78, s * 0.39)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    mod.widgets.TreeIconsBase = #(TreeIcons::script_component(vm))

    // Each field is a `DrawColor` pointing at its icon shader; the accent tint
    // is set once here and stays accent regardless of row state.
    mod.widgets.TreeIcons = set_type_default() do mod.widgets.TreeIconsBase{
        class: mod.draw.IconClass{ color: atlas.accent }
        interface: mod.draw.IconInterface{ color: atlas.accent }
        enum_type: mod.draw.IconEnum{ color: atlas.accent }
        datatype: mod.draw.IconDataType{ color: atlas.accent }
        package: mod.draw.IconPackage{ color: atlas.accent }
        diagram: mod.draw.IconDiagram{ color: atlas.accent }
        flow: mod.draw.IconFlow{ color: atlas.accent }
        sequence: mod.draw.IconSequence{ color: atlas.accent }
        note: mod.draw.IconNote{ color: atlas.accent }
        message: mod.draw.IconMessage{ color: atlas.accent }
        package_plus: mod.draw.IconPackagePlus{ color: atlas.accent }
        paintbrush: mod.draw.IconPaintbrush{ color: atlas.accent }
        pin: mod.draw.IconPin{ color: atlas.accent }
        pin_off: mod.draw.IconPinOff{ color: atlas.accent }
        share: mod.draw.IconShare{ color: atlas.accent }
        spline: mod.draw.IconSpline{ color: atlas.accent }
        spline_pointer: mod.draw.IconSplinePointer{ color: atlas.accent }
        square_minus: mod.draw.IconSquareMinus{ color: atlas.accent }
        square_plus: mod.draw.IconSquarePlus{ color: atlas.accent }
        trash: mod.draw.IconTrash{ color: atlas.accent }
        list_collapse: mod.draw.IconListCollapse{ color: atlas.accent }
        list_expand: mod.draw.IconListExpand{ color: atlas.accent }
        pencil: mod.draw.IconPencil{ color: atlas.accent }
    }
}

/// The per-kind glyph set, drawn in immediate mode via `DrawColor::draw_abs`.
/// Field order matches the `TreeIcons` DSL above.
#[derive(Script, ScriptHook)]
pub struct TreeIcons {
    #[live]
    pub class: DrawColor,
    #[live]
    pub interface: DrawColor,
    #[live]
    pub enum_type: DrawColor,
    #[live]
    pub datatype: DrawColor,
    #[live]
    pub package: DrawColor,
    #[live]
    pub diagram: DrawColor,
    #[live]
    pub flow: DrawColor,
    #[live]
    pub sequence: DrawColor,
    #[live]
    pub note: DrawColor,
    #[live]
    pub message: DrawColor,
    #[live]
    pub package_plus: DrawColor,
    #[live]
    pub paintbrush: DrawColor,
    #[live]
    pub pin: DrawColor,
    #[live]
    pub pin_off: DrawColor,
    #[live]
    pub share: DrawColor,
    #[live]
    pub spline: DrawColor,
    #[live]
    pub spline_pointer: DrawColor,
    #[live]
    pub square_minus: DrawColor,
    #[live]
    pub square_plus: DrawColor,
    #[live]
    pub trash: DrawColor,
    #[live]
    pub list_collapse: DrawColor,
    #[live]
    pub list_expand: DrawColor,
    #[live]
    pub pencil: DrawColor,
}

impl TreeIcons {
    /// All nine glyphs paired with a short label, in a stable order. Used by the
    /// `icon_harness` bin's proof-grid; the shipping tree/doc-tabs pick glyphs by
    /// `TreeKind` via `icon_for` in `tree_panel.rs` instead.
    #[allow(dead_code)]
    pub fn labeled_mut(&mut self) -> [(&'static str, &mut DrawColor); 23] {
        [
            ("class", &mut self.class),
            ("interface", &mut self.interface),
            ("enum", &mut self.enum_type),
            ("datatype", &mut self.datatype),
            ("package", &mut self.package),
            ("diagram", &mut self.diagram),
            ("flow", &mut self.flow),
            ("sequence", &mut self.sequence),
            ("note", &mut self.note),
            ("message", &mut self.message),
            ("package+", &mut self.package_plus),
            ("paintbrush", &mut self.paintbrush),
            ("pin", &mut self.pin),
            ("pin-off", &mut self.pin_off),
            ("share", &mut self.share),
            ("spline", &mut self.spline),
            ("spline-ptr", &mut self.spline_pointer),
            ("square-minus", &mut self.square_minus),
            ("square-plus", &mut self.square_plus),
            ("trash", &mut self.trash),
            ("collapse", &mut self.list_collapse),
            ("expand", &mut self.list_expand),
            ("pencil", &mut self.pencil),
        ]
    }
}
