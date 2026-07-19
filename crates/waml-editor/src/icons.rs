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
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.8750, s * 0.3333)
            sdf.arc_to(s * 0.7917, s * 0.3334, s * 0.0833, -0.0010, -1.0472)
            sdf.line_to(s * 0.5417, s * 0.0946)
            sdf.arc_to(s * 0.5000, s * 0.1668, s * 0.0833, -1.0472, -2.0944)
            sdf.line_to(s * 0.1667, s * 0.2613)
            sdf.arc_to(s * 0.2083, s * 0.3334, s * 0.0833, -2.0944, -3.1406)
            sdf.line_to(s * 0.1250, s * 0.6667)
            sdf.arc_to(s * 0.2083, s * 0.6666, s * 0.0833, 3.1406, 2.0944)
            sdf.line_to(s * 0.4583, s * 0.9054)
            sdf.arc_to(s * 0.5000, s * 0.8332, s * 0.0833, 2.0944, 1.0472)
            sdf.line_to(s * 0.8333, s * 0.7388)
            sdf.arc_to(s * 0.7917, s * 0.6666, s * 0.0833, 1.0472, 0.0010)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.1375, s * 0.2917)
            sdf.line_to(s * 0.5000, s * 0.5000)
            sdf.line_to(s * 0.8625, s * 0.2917)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.5000, s * 0.9167)
            sdf.line_to(s * 0.5000, s * 0.5000)
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

    // Message: speech bubble with three text lines.
    // Faithful port of resources/icons/message-square-text.svg via scripts/gen-icon.py.
    mod.draw.IconMessage = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.9167, s * 0.7083)
            sdf.arc_to(s * 0.8333, s * 0.7083, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.2845, s * 0.7917)
            sdf.arc_to(s * 0.2845, s * 0.8750, s * 0.0833, -1.5710, -2.3563)
            sdf.line_to(s * 0.1338, s * 0.9078)
            sdf.arc_to(s * 0.1129, s * 0.8869, s * 0.0296, 0.7855, 3.1415)
            sdf.line_to(s * 0.0833, s * 0.2083)
            sdf.arc_to(s * 0.1667, s * 0.2083, s * 0.0833, 3.1416, 4.7124)
            sdf.line_to(s * 0.8333, s * 0.1250)
            sdf.arc_to(s * 0.8333, s * 0.2083, s * 0.0833, -1.5708, 0.0000)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2917, s * 0.4583)
            sdf.line_to(s * 0.7083, s * 0.4583)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2917, s * 0.6250)
            sdf.line_to(s * 0.5417, s * 0.6250)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2917, s * 0.2917)
            sdf.line_to(s * 0.6250, s * 0.2917)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Package plus: box with a + badge.
    // Faithful port of resources/icons/package-plus.svg via scripts/gen-icon.py.
    mod.draw.IconPackagePlus = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.5000, s * 0.9167)
            sdf.line_to(s * 0.5000, s * 0.5000)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6667, s * 0.7083)
            sdf.line_to(s * 0.9167, s * 0.7083)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.7917, s * 0.5833)
            sdf.line_to(s * 0.7917, s * 0.8333)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.8750, s * 0.4390)
            sdf.line_to(s * 0.8750, s * 0.3333)
            sdf.arc_to(s * 0.7917, s * 0.3334, s * 0.0833, -0.0010, -1.0472)
            sdf.line_to(s * 0.5417, s * 0.0946)
            sdf.arc_to(s * 0.5000, s * 0.1668, s * 0.0833, -1.0472, -2.0944)
            sdf.line_to(s * 0.1667, s * 0.2613)
            sdf.arc_to(s * 0.2083, s * 0.3334, s * 0.0833, -2.0944, -3.1406)
            sdf.line_to(s * 0.1250, s * 0.6667)
            sdf.arc_to(s * 0.2083, s * 0.6665, s * 0.0833, 3.1401, 2.0944)
            sdf.line_to(s * 0.4583, s * 0.9054)
            sdf.arc_to(s * 0.5000, s * 0.8332, s * 0.0833, 2.0949, 1.0477)
            sdf.line_to(s * 0.6115, s * 0.8656)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.1371, s * 0.2917)
            sdf.line_to(s * 0.5000, s * 0.5000)
            sdf.line_to(s * 0.8629, s * 0.2917)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.3125, s * 0.1779)
            sdf.line_to(s * 0.6874, s * 0.3924)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Paintbrush (vertical): bristle head + handle.
    // Faithful port of resources/icons/paintbrush-vertical.svg via scripts/gen-icon.py.
    mod.draw.IconPaintbrush = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.4167, s * 0.0833)
            sdf.line_to(s * 0.4167, s * 0.1667)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.5833, s * 0.0833)
            sdf.line_to(s * 0.5833, s * 0.2500)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.7083, s * 0.0833)
            sdf.arc_to(s * 0.7083, s * 0.1250, s * 0.0417, -1.5708, 0.0000)
            sdf.line_to(s * 0.7500, s * 0.5000)
            sdf.line_to(s * 0.2500, s * 0.5000)
            sdf.line_to(s * 0.2500, s * 0.1250)
            sdf.arc_to(s * 0.2917, s * 0.1250, s * 0.0417, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2500, s * 0.5000)
            sdf.arc_to(s * 0.2500, s * 0.5417, s * 0.0417, -1.5708, -3.1416)
            sdf.line_to(s * 0.2083, s * 0.5833)
            sdf.arc_to(s * 0.2917, s * 0.5833, s * 0.0833, 3.1416, 1.5708)
            sdf.line_to(s * 0.3750, s * 0.6667)
            sdf.arc_to(s * 0.3750, s * 0.7083, s * 0.0417, -1.5708, 0.0000)
            sdf.line_to(s * 0.4167, s * 0.8292)
            sdf.arc_to(s * 0.5000, s * 0.8292, s * 0.0833, 3.1416, 0.0000)
            sdf.line_to(s * 0.5833, s * 0.7083)
            sdf.arc_to(s * 0.6250, s * 0.7083, s * 0.0417, 3.1416, 4.7124)
            sdf.line_to(s * 0.7083, s * 0.6667)
            sdf.arc_to(s * 0.7083, s * 0.5833, s * 0.0833, 1.5708, 0.0000)
            sdf.line_to(s * 0.7917, s * 0.5417)
            sdf.arc_to(s * 0.7500, s * 0.5417, s * 0.0417, 0.0000, -1.5708)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Pin: map pin.
    // Faithful port of resources/icons/pin.svg via scripts/gen-icon.py.
    mod.draw.IconPin = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.5000, s * 0.7083)
            sdf.line_to(s * 0.5000, s * 0.9167)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.3750, s * 0.4483)
            sdf.arc_to(s * 0.2917, s * 0.4483, s * 0.0833, 0.0005, 1.1096)
            sdf.line_to(s * 0.2546, s * 0.5604)
            sdf.arc_to(s * 0.2917, s * 0.6350, s * 0.0833, -2.0320, -3.1411)
            sdf.line_to(s * 0.2083, s * 0.6667)
            sdf.arc_to(s * 0.2500, s * 0.6667, s * 0.0417, 3.1416, 1.5708)
            sdf.line_to(s * 0.7500, s * 0.7083)
            sdf.arc_to(s * 0.7500, s * 0.6667, s * 0.0417, 1.5708, 0.0000)
            sdf.line_to(s * 0.7917, s * 0.6350)
            sdf.arc_to(s * 0.7083, s * 0.6350, s * 0.0833, -0.0005, -1.1096)
            sdf.line_to(s * 0.6713, s * 0.5229)
            sdf.arc_to(s * 0.7083, s * 0.4483, s * 0.0833, 2.0320, 3.1411)
            sdf.line_to(s * 0.6250, s * 0.2917)
            sdf.arc_to(s * 0.6667, s * 0.2917, s * 0.0417, 3.1416, 4.7124)
            sdf.arc_to(s * 0.6667, s * 0.1667, s * 0.0833, 1.5708, -1.5708)
            sdf.line_to(s * 0.3333, s * 0.0833)
            sdf.arc_to(s * 0.3333, s * 0.1667, s * 0.0833, -1.5708, -4.7124)
            sdf.arc_to(s * 0.3333, s * 0.2917, s * 0.0417, -1.5708, 0.0000)
            sdf.close_path()
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Pin off: map pin with a slash.
    // Faithful port of resources/icons/pin-off.svg via scripts/gen-icon.py.
    mod.draw.IconPinOff = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.5000, s * 0.7083)
            sdf.line_to(s * 0.5000, s * 0.9167)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6250, s * 0.3892)
            sdf.line_to(s * 0.6250, s * 0.2917)
            sdf.arc_to(s * 0.6667, s * 0.2917, s * 0.0417, 3.1416, 4.7124)
            sdf.arc_to(s * 0.6667, s * 0.1667, s * 0.0833, 1.5708, -1.5708)
            sdf.line_to(s * 0.3288, s * 0.0833)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.0833, s * 0.0833)
            sdf.line_to(s * 0.9167, s * 0.9167)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.3750, s * 0.3750)
            sdf.line_to(s * 0.3750, s * 0.4483)
            sdf.arc_to(s * 0.2917, s * 0.4483, s * 0.0833, 0.0005, 1.1096)
            sdf.line_to(s * 0.2546, s * 0.5604)
            sdf.arc_to(s * 0.2917, s * 0.6350, s * 0.0833, -2.0320, -3.1411)
            sdf.line_to(s * 0.2083, s * 0.6667)
            sdf.arc_to(s * 0.2500, s * 0.6667, s * 0.0417, 3.1416, 1.5708)
            sdf.line_to(s * 0.7083, s * 0.7083)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Share: node-link share glyph.
    // Faithful port of resources/icons/share.svg via scripts/gen-icon.py.
    mod.draw.IconShare = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.5000, s * 0.0833)
            sdf.line_to(s * 0.5000, s * 0.6250)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6667, s * 0.2500)
            sdf.line_to(s * 0.5000, s * 0.0833)
            sdf.line_to(s * 0.3333, s * 0.2500)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.1667, s * 0.5000)
            sdf.line_to(s * 0.1667, s * 0.8333)
            sdf.arc_to(s * 0.2500, s * 0.8333, s * 0.0833, 3.1416, 1.5708)
            sdf.line_to(s * 0.7500, s * 0.9167)
            sdf.arc_to(s * 0.7500, s * 0.8333, s * 0.0833, 1.5708, 0.0000)
            sdf.line_to(s * 0.8333, s * 0.5000)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Spline: curve with control handles.
    // Faithful port of resources/icons/spline.svg via scripts/gen-icon.py.
    mod.draw.IconSpline = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.8750, s * 0.2083)
            sdf.arc_to(s * 0.7917, s * 0.2083, s * 0.0833, 0.0000, 3.1416)
            sdf.arc_to(s * 0.7917, s * 0.2083, s * 0.0833, 3.1416, 6.2832)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2917, s * 0.7917)
            sdf.arc_to(s * 0.2083, s * 0.7917, s * 0.0833, 0.0000, 3.1416)
            sdf.arc_to(s * 0.2083, s * 0.7917, s * 0.0833, 3.1416, 6.2832)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2083, s * 0.7083)
            sdf.arc_to(s * 0.7083, s * 0.7083, s * 0.5000, 3.1416, 4.7124)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Spline pointer: spline meeting a cursor.
    // Faithful port of resources/icons/spline-pointer.svg via scripts/gen-icon.py.
    mod.draw.IconSplinePointer = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.5014, s * 0.5284)
            sdf.arc_to(s * 0.5207, s * 0.5207, s * 0.0208, 2.7623, 5.0917)
            sdf.line_to(s * 0.9034, s * 0.6473)
            sdf.arc_to(s * 0.8958, s * 0.6667, s * 0.0208, -1.1983, 1.2683)
            sdf.line_to(s * 0.7585, s * 0.7310)
            sdf.arc_to(s * 0.7708, s * 0.7708, s * 0.0417, -1.8706, -2.8417)
            sdf.line_to(s * 0.6865, s * 0.9020)
            sdf.arc_to(s * 0.6667, s * 0.8958, s * 0.0208, 0.3025, 2.7691)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2083, s * 0.7083)
            sdf.arc_to(s * 0.7083, s * 0.7083, s * 0.5000, 3.1416, 4.7124)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.8750, s * 0.2083)
            sdf.arc_to(s * 0.7917, s * 0.2083, s * 0.0833, 0.0000, 3.1416)
            sdf.arc_to(s * 0.7917, s * 0.2083, s * 0.0833, 3.1416, 6.2832)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2917, s * 0.7917)
            sdf.arc_to(s * 0.2083, s * 0.7917, s * 0.0833, 0.0000, 3.1416)
            sdf.arc_to(s * 0.2083, s * 0.7917, s * 0.0833, 3.1416, 6.2832)
            sdf.close_path()
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Square minus: rounded square with a minus.
    // Faithful port of resources/icons/square-minus.svg via scripts/gen-icon.py.
    mod.draw.IconSquareMinus = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.2083, s * 0.1250)
            sdf.line_to(s * 0.7917, s * 0.1250)
            sdf.arc_to(s * 0.7917, s * 0.2083, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.8750, s * 0.7917)
            sdf.arc_to(s * 0.7917, s * 0.7917, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.2083, s * 0.8750)
            sdf.arc_to(s * 0.2083, s * 0.7917, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.1250, s * 0.2083)
            sdf.arc_to(s * 0.2083, s * 0.2083, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.3333, s * 0.5000)
            sdf.line_to(s * 0.6667, s * 0.5000)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Square plus: rounded square with a plus.
    // Faithful port of resources/icons/square-plus.svg via scripts/gen-icon.py.
    mod.draw.IconSquarePlus = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.2083, s * 0.1250)
            sdf.line_to(s * 0.7917, s * 0.1250)
            sdf.arc_to(s * 0.7917, s * 0.2083, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.8750, s * 0.7917)
            sdf.arc_to(s * 0.7917, s * 0.7917, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.2083, s * 0.8750)
            sdf.arc_to(s * 0.2083, s * 0.7917, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.1250, s * 0.2083)
            sdf.arc_to(s * 0.2083, s * 0.2083, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.3333, s * 0.5000)
            sdf.line_to(s * 0.6667, s * 0.5000)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.5000, s * 0.3333)
            sdf.line_to(s * 0.5000, s * 0.6667)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Trash: waste bin.
    // Faithful port of resources/icons/trash.svg via scripts/gen-icon.py.
    mod.draw.IconTrash = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.7917, s * 0.2500)
            sdf.line_to(s * 0.7917, s * 0.8333)
            sdf.arc_to(s * 0.7083, s * 0.8333, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.2917, s * 0.9167)
            sdf.arc_to(s * 0.2917, s * 0.8333, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.2083, s * 0.2500)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.1250, s * 0.2500)
            sdf.line_to(s * 0.8750, s * 0.2500)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.3333, s * 0.2500)
            sdf.line_to(s * 0.3333, s * 0.1667)
            sdf.arc_to(s * 0.4167, s * 0.1667, s * 0.0833, 3.1416, 4.7124)
            sdf.line_to(s * 0.5833, s * 0.0833)
            sdf.arc_to(s * 0.5833, s * 0.1667, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.6667, s * 0.2500)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // List collapse (down-up): rows + inward chevrons.
    // Faithful port of resources/icons/list-chevrons-down-up.svg via scripts/gen-icon.py.
    mod.draw.IconListCollapse = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.1250, s * 0.2083)
            sdf.line_to(s * 0.4583, s * 0.2083)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.1250, s * 0.5000)
            sdf.line_to(s * 0.4583, s * 0.5000)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.1250, s * 0.7917)
            sdf.line_to(s * 0.4583, s * 0.7917)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6250, s * 0.2083)
            sdf.line_to(s * 0.7500, s * 0.3333)
            sdf.line_to(s * 0.8750, s * 0.2083)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6250, s * 0.7917)
            sdf.line_to(s * 0.7500, s * 0.6667)
            sdf.line_to(s * 0.8750, s * 0.7917)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // List expand (up-down): rows + outward chevrons.
    // Faithful port of resources/icons/list-chevrons-up-down.svg via scripts/gen-icon.py.
    mod.draw.IconListExpand = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.1250, s * 0.2083)
            sdf.line_to(s * 0.4583, s * 0.2083)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.1250, s * 0.5000)
            sdf.line_to(s * 0.4583, s * 0.5000)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.1250, s * 0.7917)
            sdf.line_to(s * 0.4583, s * 0.7917)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6250, s * 0.3333)
            sdf.line_to(s * 0.7500, s * 0.2083)
            sdf.line_to(s * 0.8750, s * 0.3333)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6250, s * 0.6667)
            sdf.line_to(s * 0.7500, s * 0.7917)
            sdf.line_to(s * 0.8750, s * 0.6667)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Pencil.
    // Faithful port of resources/icons/pencil.svg via scripts/gen-icon.py.
    mod.draw.IconPencil = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.8823, s * 0.2838)
            sdf.arc_to(s * 0.7992, s * 0.2008, s * 0.1175, 0.7855, -2.3561)
            sdf.line_to(s * 0.1601, s * 0.6739)
            sdf.arc_to(s * 0.2189, s * 0.7329, s * 0.0833, -2.3547, -2.8441)
            sdf.line_to(s * 0.0842, s * 0.8898)
            sdf.arc_to(s * 0.1042, s * 0.8958, s * 0.0208, -2.8512, -5.0044)
            sdf.line_to(s * 0.2915, s * 0.8608)
            sdf.arc_to(s * 0.2673, s * 0.7810, s * 0.0833, 1.2755, 0.7870)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6250, s * 0.2083)
            sdf.line_to(s * 0.7917, s * 0.3750)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Menu (hamburger): three rows.
    // Faithful port of resources/icons/menu.svg via scripts/gen-icon.py.
    mod.draw.IconMenu = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.1667, s * 0.2083)
            sdf.line_to(s * 0.8333, s * 0.2083)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.1667, s * 0.5000)
            sdf.line_to(s * 0.8333, s * 0.5000)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.1667, s * 0.7917)
            sdf.line_to(s * 0.8333, s * 0.7917)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Moon: crescent.
    // Faithful port of resources/icons/moon.svg via scripts/gen-icon.py.
    mod.draw.IconMoon = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.8744, s * 0.5203)
            sdf.arc_to(s * 0.4999, s * 0.5000, s * 0.3750, 0.0539, 4.6584)
            sdf.line_to(s * 0.4837, s * 0.1258)
            sdf.line_to(s * 0.4874, s * 0.1268)
            sdf.line_to(s * 0.4907, s * 0.1284)
            sdf.line_to(s * 0.4935, s * 0.1307)
            sdf.line_to(s * 0.4959, s * 0.1334)
            sdf.line_to(s * 0.4977, s * 0.1366)
            sdf.line_to(s * 0.4991, s * 0.1401)
            sdf.line_to(s * 0.4998, s * 0.1438)
            sdf.line_to(s * 0.5000, s * 0.1477)
            sdf.line_to(s * 0.4995, s * 0.1515)
            sdf.line_to(s * 0.4983, s * 0.1554)
            sdf.line_to(s * 0.4964, s * 0.1590)
            sdf.arc_to(s * 0.7084, s * 0.2915, s * 0.2500, -2.5830, -5.2710)
            sdf.line_to(s * 0.8446, s * 0.5017)
            sdf.line_to(s * 0.8484, s * 0.5005)
            sdf.line_to(s * 0.8523, s * 0.5000)
            sdf.line_to(s * 0.8562, s * 0.5001)
            sdf.line_to(s * 0.8599, s * 0.5009)
            sdf.line_to(s * 0.8634, s * 0.5022)
            sdf.line_to(s * 0.8665, s * 0.5041)
            sdf.line_to(s * 0.8693, s * 0.5064)
            sdf.line_to(s * 0.8716, s * 0.5093)
            sdf.line_to(s * 0.8732, s * 0.5125)
            sdf.line_to(s * 0.8742, s * 0.5162)
            sdf.line_to(s * 0.8744, s * 0.5203)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/align-center-horizontal.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/align-center-horizontal.svg via scripts/gen-icon.py.
    mod.draw.IconAlignCenterHorizontal = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.0833, s * 0.5000)
            sdf.line_to(s * 0.9167, s * 0.5000)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.4167, s * 0.6667)
            sdf.line_to(s * 0.4167, s * 0.8333)
            sdf.arc_to(s * 0.3333, s * 0.8333, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.2500, s * 0.9167)
            sdf.arc_to(s * 0.2500, s * 0.8333, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.1667, s * 0.6667)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.4167, s * 0.3333)
            sdf.line_to(s * 0.4167, s * 0.1667)
            sdf.arc_to(s * 0.3333, s * 0.1667, s * 0.0833, 0.0000, -1.5708)
            sdf.line_to(s * 0.2500, s * 0.0833)
            sdf.arc_to(s * 0.2500, s * 0.1667, s * 0.0833, -1.5708, -3.1416)
            sdf.line_to(s * 0.1667, s * 0.3333)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.8333, s * 0.6667)
            sdf.line_to(s * 0.8333, s * 0.7083)
            sdf.arc_to(s * 0.7500, s * 0.7083, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.6667, s * 0.7917)
            sdf.arc_to(s * 0.6667, s * 0.7083, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.5833, s * 0.6667)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.5833, s * 0.3333)
            sdf.line_to(s * 0.5833, s * 0.2917)
            sdf.line_to(s * 0.5841, s * 0.2804)
            sdf.line_to(s * 0.5863, s * 0.2696)
            sdf.line_to(s * 0.5899, s * 0.2593)
            sdf.line_to(s * 0.5948, s * 0.2497)
            sdf.line_to(s * 0.6008, s * 0.2408)
            sdf.line_to(s * 0.6078, s * 0.2328)
            sdf.line_to(s * 0.6158, s * 0.2258)
            sdf.line_to(s * 0.6247, s * 0.2198)
            sdf.line_to(s * 0.6343, s * 0.2149)
            sdf.line_to(s * 0.6446, s * 0.2113)
            sdf.line_to(s * 0.6554, s * 0.2091)
            sdf.line_to(s * 0.6667, s * 0.2083)
            sdf.line_to(s * 0.7500, s * 0.2083)
            sdf.arc_to(s * 0.7500, s * 0.2917, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.8333, s * 0.3333)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/align-center-vertical.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/align-center-vertical.svg via scripts/gen-icon.py.
    mod.draw.IconAlignCenterVertical = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.5000, s * 0.0833)
            sdf.line_to(s * 0.5000, s * 0.9167)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.3333, s * 0.4167)
            sdf.line_to(s * 0.1667, s * 0.4167)
            sdf.arc_to(s * 0.1667, s * 0.3333, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.0833, s * 0.2500)
            sdf.line_to(s * 0.0841, s * 0.2387)
            sdf.line_to(s * 0.0863, s * 0.2279)
            sdf.line_to(s * 0.0899, s * 0.2176)
            sdf.line_to(s * 0.0948, s * 0.2080)
            sdf.line_to(s * 0.1008, s * 0.1992)
            sdf.line_to(s * 0.1078, s * 0.1911)
            sdf.line_to(s * 0.1158, s * 0.1841)
            sdf.line_to(s * 0.1247, s * 0.1781)
            sdf.line_to(s * 0.1343, s * 0.1732)
            sdf.line_to(s * 0.1446, s * 0.1697)
            sdf.line_to(s * 0.1554, s * 0.1674)
            sdf.line_to(s * 0.1667, s * 0.1667)
            sdf.line_to(s * 0.3333, s * 0.1667)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6667, s * 0.4167)
            sdf.line_to(s * 0.8333, s * 0.4167)
            sdf.arc_to(s * 0.8333, s * 0.3333, s * 0.0833, 1.5708, 0.0000)
            sdf.line_to(s * 0.9167, s * 0.2500)
            sdf.arc_to(s * 0.8333, s * 0.2500, s * 0.0833, 0.0000, -1.5708)
            sdf.line_to(s * 0.6667, s * 0.1667)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.3333, s * 0.8333)
            sdf.line_to(s * 0.2917, s * 0.8333)
            sdf.arc_to(s * 0.2917, s * 0.7500, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.2083, s * 0.6667)
            sdf.line_to(s * 0.2091, s * 0.6554)
            sdf.line_to(s * 0.2113, s * 0.6446)
            sdf.line_to(s * 0.2149, s * 0.6343)
            sdf.line_to(s * 0.2198, s * 0.6247)
            sdf.line_to(s * 0.2258, s * 0.6158)
            sdf.line_to(s * 0.2328, s * 0.6078)
            sdf.line_to(s * 0.2408, s * 0.6008)
            sdf.line_to(s * 0.2497, s * 0.5948)
            sdf.line_to(s * 0.2593, s * 0.5899)
            sdf.line_to(s * 0.2696, s * 0.5863)
            sdf.line_to(s * 0.2804, s * 0.5841)
            sdf.line_to(s * 0.2917, s * 0.5833)
            sdf.line_to(s * 0.3333, s * 0.5833)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6667, s * 0.5833)
            sdf.line_to(s * 0.7083, s * 0.5833)
            sdf.arc_to(s * 0.7083, s * 0.6667, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.7917, s * 0.7500)
            sdf.arc_to(s * 0.7083, s * 0.7500, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.6667, s * 0.8333)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/align-end-horizontal.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/align-end-horizontal.svg via scripts/gen-icon.py.
    mod.draw.IconAlignEndHorizontal = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.2500, s * 0.0833)
            sdf.line_to(s * 0.3333, s * 0.0833)
            sdf.arc_to(s * 0.3333, s * 0.1667, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.4167, s * 0.6667)
            sdf.arc_to(s * 0.3333, s * 0.6667, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.2500, s * 0.7500)
            sdf.arc_to(s * 0.2500, s * 0.6667, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.1667, s * 0.1667)
            sdf.arc_to(s * 0.2500, s * 0.1667, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6667, s * 0.3750)
            sdf.line_to(s * 0.7500, s * 0.3750)
            sdf.arc_to(s * 0.7500, s * 0.4583, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.8333, s * 0.6667)
            sdf.arc_to(s * 0.7500, s * 0.6667, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.6667, s * 0.7500)
            sdf.arc_to(s * 0.6667, s * 0.6667, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.5833, s * 0.4583)
            sdf.arc_to(s * 0.6667, s * 0.4583, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.9167, s * 0.9167)
            sdf.line_to(s * 0.0833, s * 0.9167)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/align-horizontal-distribute-center.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/align-horizontal-distribute-center.svg via scripts/gen-icon.py.
    mod.draw.IconAlignHorizontalDistributeCenter = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.2500, s * 0.2083)
            sdf.line_to(s * 0.3333, s * 0.2083)
            sdf.arc_to(s * 0.3333, s * 0.2917, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.4167, s * 0.7083)
            sdf.arc_to(s * 0.3333, s * 0.7083, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.2500, s * 0.7917)
            sdf.arc_to(s * 0.2500, s * 0.7083, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.1667, s * 0.2917)
            sdf.arc_to(s * 0.2500, s * 0.2917, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6667, s * 0.2917)
            sdf.line_to(s * 0.7500, s * 0.2917)
            sdf.arc_to(s * 0.7500, s * 0.3750, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.8333, s * 0.6250)
            sdf.arc_to(s * 0.7500, s * 0.6250, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.6667, s * 0.7083)
            sdf.arc_to(s * 0.6667, s * 0.6250, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.5833, s * 0.3750)
            sdf.arc_to(s * 0.6667, s * 0.3750, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.7083, s * 0.9167)
            sdf.line_to(s * 0.7083, s * 0.7083)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.7083, s * 0.2917)
            sdf.line_to(s * 0.7083, s * 0.0833)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2917, s * 0.9167)
            sdf.line_to(s * 0.2917, s * 0.7917)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2917, s * 0.2083)
            sdf.line_to(s * 0.2917, s * 0.0833)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/align-horizontal-distribute-end.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/align-horizontal-distribute-end.svg via scripts/gen-icon.py.
    mod.draw.IconAlignHorizontalDistributeEnd = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.2500, s * 0.2083)
            sdf.line_to(s * 0.3333, s * 0.2083)
            sdf.arc_to(s * 0.3333, s * 0.2917, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.4167, s * 0.7083)
            sdf.arc_to(s * 0.3333, s * 0.7083, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.2500, s * 0.7917)
            sdf.arc_to(s * 0.2500, s * 0.7083, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.1667, s * 0.2917)
            sdf.arc_to(s * 0.2500, s * 0.2917, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6667, s * 0.2917)
            sdf.line_to(s * 0.7500, s * 0.2917)
            sdf.arc_to(s * 0.7500, s * 0.3750, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.8333, s * 0.6250)
            sdf.arc_to(s * 0.7500, s * 0.6250, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.6667, s * 0.7083)
            sdf.arc_to(s * 0.6667, s * 0.6250, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.5833, s * 0.3750)
            sdf.arc_to(s * 0.6667, s * 0.3750, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.4167, s * 0.0833)
            sdf.line_to(s * 0.4167, s * 0.9167)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.8333, s * 0.0833)
            sdf.line_to(s * 0.8333, s * 0.9167)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/align-horizontal-distribute-start.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/align-horizontal-distribute-start.svg via scripts/gen-icon.py.
    mod.draw.IconAlignHorizontalDistributeStart = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.2500, s * 0.2083)
            sdf.line_to(s * 0.3333, s * 0.2083)
            sdf.arc_to(s * 0.3333, s * 0.2917, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.4167, s * 0.7083)
            sdf.arc_to(s * 0.3333, s * 0.7083, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.2500, s * 0.7917)
            sdf.arc_to(s * 0.2500, s * 0.7083, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.1667, s * 0.2917)
            sdf.arc_to(s * 0.2500, s * 0.2917, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6667, s * 0.2917)
            sdf.line_to(s * 0.7500, s * 0.2917)
            sdf.arc_to(s * 0.7500, s * 0.3750, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.8333, s * 0.6250)
            sdf.arc_to(s * 0.7500, s * 0.6250, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.6667, s * 0.7083)
            sdf.arc_to(s * 0.6667, s * 0.6250, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.5833, s * 0.3750)
            sdf.arc_to(s * 0.6667, s * 0.3750, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.1667, s * 0.0833)
            sdf.line_to(s * 0.1667, s * 0.9167)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.5833, s * 0.0833)
            sdf.line_to(s * 0.5833, s * 0.9167)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/align-horizontal-justify-center.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/align-horizontal-justify-center.svg via scripts/gen-icon.py.
    mod.draw.IconAlignHorizontalJustifyCenter = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.1667, s * 0.2083)
            sdf.line_to(s * 0.2500, s * 0.2083)
            sdf.arc_to(s * 0.2500, s * 0.2917, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.3333, s * 0.7083)
            sdf.arc_to(s * 0.2500, s * 0.7083, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.1667, s * 0.7917)
            sdf.arc_to(s * 0.1667, s * 0.7083, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.0833, s * 0.2917)
            sdf.arc_to(s * 0.1667, s * 0.2917, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.7500, s * 0.2917)
            sdf.line_to(s * 0.8333, s * 0.2917)
            sdf.arc_to(s * 0.8333, s * 0.3750, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.9167, s * 0.6250)
            sdf.arc_to(s * 0.8333, s * 0.6250, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.7500, s * 0.7083)
            sdf.arc_to(s * 0.7500, s * 0.6250, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.6667, s * 0.3750)
            sdf.arc_to(s * 0.7500, s * 0.3750, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.5000, s * 0.0833)
            sdf.line_to(s * 0.5000, s * 0.9167)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/align-horizontal-justify-end.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/align-horizontal-justify-end.svg via scripts/gen-icon.py.
    mod.draw.IconAlignHorizontalJustifyEnd = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.1667, s * 0.2083)
            sdf.line_to(s * 0.2500, s * 0.2083)
            sdf.arc_to(s * 0.2500, s * 0.2917, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.3333, s * 0.7083)
            sdf.arc_to(s * 0.2500, s * 0.7083, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.1667, s * 0.7917)
            sdf.arc_to(s * 0.1667, s * 0.7083, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.0833, s * 0.2917)
            sdf.arc_to(s * 0.1667, s * 0.2917, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.5833, s * 0.2917)
            sdf.line_to(s * 0.6667, s * 0.2917)
            sdf.arc_to(s * 0.6667, s * 0.3750, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.7500, s * 0.6250)
            sdf.arc_to(s * 0.6667, s * 0.6250, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.5833, s * 0.7083)
            sdf.arc_to(s * 0.5833, s * 0.6250, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.5000, s * 0.3750)
            sdf.arc_to(s * 0.5833, s * 0.3750, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.9167, s * 0.0833)
            sdf.line_to(s * 0.9167, s * 0.9167)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/align-horizontal-justify-start.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/align-horizontal-justify-start.svg via scripts/gen-icon.py.
    mod.draw.IconAlignHorizontalJustifyStart = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.3333, s * 0.2083)
            sdf.line_to(s * 0.4167, s * 0.2083)
            sdf.arc_to(s * 0.4167, s * 0.2917, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.5000, s * 0.7083)
            sdf.arc_to(s * 0.4167, s * 0.7083, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.3333, s * 0.7917)
            sdf.arc_to(s * 0.3333, s * 0.7083, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.2500, s * 0.2917)
            sdf.arc_to(s * 0.3333, s * 0.2917, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.7500, s * 0.2917)
            sdf.line_to(s * 0.8333, s * 0.2917)
            sdf.arc_to(s * 0.8333, s * 0.3750, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.9167, s * 0.6250)
            sdf.arc_to(s * 0.8333, s * 0.6250, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.7500, s * 0.7083)
            sdf.arc_to(s * 0.7500, s * 0.6250, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.6667, s * 0.3750)
            sdf.arc_to(s * 0.7500, s * 0.3750, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.0833, s * 0.0833)
            sdf.line_to(s * 0.0833, s * 0.9167)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/align-horizontal-space-around.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/align-horizontal-space-around.svg via scripts/gen-icon.py.
    mod.draw.IconAlignHorizontalSpaceAround = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.4583, s * 0.2917)
            sdf.line_to(s * 0.5417, s * 0.2917)
            sdf.arc_to(s * 0.5417, s * 0.3750, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.6250, s * 0.6250)
            sdf.arc_to(s * 0.5417, s * 0.6250, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.4583, s * 0.7083)
            sdf.arc_to(s * 0.4583, s * 0.6250, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.3750, s * 0.3750)
            sdf.arc_to(s * 0.4583, s * 0.3750, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.1667, s * 0.9167)
            sdf.line_to(s * 0.1667, s * 0.0833)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.8333, s * 0.9167)
            sdf.line_to(s * 0.8333, s * 0.0833)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/align-horizontal-space-between.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/align-horizontal-space-between.svg via scripts/gen-icon.py.
    mod.draw.IconAlignHorizontalSpaceBetween = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.2083, s * 0.2083)
            sdf.line_to(s * 0.2917, s * 0.2083)
            sdf.arc_to(s * 0.2917, s * 0.2917, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.3750, s * 0.7083)
            sdf.arc_to(s * 0.2917, s * 0.7083, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.2083, s * 0.7917)
            sdf.arc_to(s * 0.2083, s * 0.7083, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.1250, s * 0.2917)
            sdf.arc_to(s * 0.2083, s * 0.2917, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.7083, s * 0.2917)
            sdf.line_to(s * 0.7917, s * 0.2917)
            sdf.arc_to(s * 0.7917, s * 0.3750, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.8750, s * 0.6250)
            sdf.arc_to(s * 0.7917, s * 0.6250, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.7083, s * 0.7083)
            sdf.arc_to(s * 0.7083, s * 0.6250, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.6250, s * 0.3750)
            sdf.arc_to(s * 0.7083, s * 0.3750, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.1250, s * 0.0833)
            sdf.line_to(s * 0.1250, s * 0.9167)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.8750, s * 0.0833)
            sdf.line_to(s * 0.8750, s * 0.9167)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/align-start-horizontal.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/align-start-horizontal.svg via scripts/gen-icon.py.
    mod.draw.IconAlignStartHorizontal = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.2500, s * 0.2500)
            sdf.line_to(s * 0.3333, s * 0.2500)
            sdf.arc_to(s * 0.3333, s * 0.3333, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.4167, s * 0.8333)
            sdf.arc_to(s * 0.3333, s * 0.8333, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.2500, s * 0.9167)
            sdf.arc_to(s * 0.2500, s * 0.8333, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.1667, s * 0.3333)
            sdf.arc_to(s * 0.2500, s * 0.3333, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6667, s * 0.2500)
            sdf.line_to(s * 0.7500, s * 0.2500)
            sdf.arc_to(s * 0.7500, s * 0.3333, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.8333, s * 0.5417)
            sdf.arc_to(s * 0.7500, s * 0.5417, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.6667, s * 0.6250)
            sdf.arc_to(s * 0.6667, s * 0.5417, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.5833, s * 0.3333)
            sdf.arc_to(s * 0.6667, s * 0.3333, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.9167, s * 0.0833)
            sdf.line_to(s * 0.0833, s * 0.0833)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/align-start-vertical.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/align-start-vertical.svg via scripts/gen-icon.py.
    mod.draw.IconAlignStartVertical = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.3333, s * 0.5833)
            sdf.line_to(s * 0.5417, s * 0.5833)
            sdf.arc_to(s * 0.5417, s * 0.6667, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.6250, s * 0.7500)
            sdf.arc_to(s * 0.5417, s * 0.7500, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.3333, s * 0.8333)
            sdf.arc_to(s * 0.3333, s * 0.7500, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.2500, s * 0.6667)
            sdf.arc_to(s * 0.3333, s * 0.6667, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.3333, s * 0.1667)
            sdf.line_to(s * 0.8333, s * 0.1667)
            sdf.arc_to(s * 0.8333, s * 0.2500, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.9167, s * 0.3333)
            sdf.arc_to(s * 0.8333, s * 0.3333, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.3333, s * 0.4167)
            sdf.arc_to(s * 0.3333, s * 0.3333, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.2500, s * 0.2500)
            sdf.arc_to(s * 0.3333, s * 0.2500, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.0833, s * 0.0833)
            sdf.line_to(s * 0.0833, s * 0.9167)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/align-vertical-distribute-center.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/align-vertical-distribute-center.svg via scripts/gen-icon.py.
    mod.draw.IconAlignVerticalDistributeCenter = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.9167, s * 0.7083)
            sdf.line_to(s * 0.7917, s * 0.7083)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.9167, s * 0.2917)
            sdf.line_to(s * 0.7083, s * 0.2917)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2083, s * 0.7083)
            sdf.line_to(s * 0.0833, s * 0.7083)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2917, s * 0.2917)
            sdf.line_to(s * 0.0833, s * 0.2917)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2917, s * 0.5833)
            sdf.line_to(s * 0.7083, s * 0.5833)
            sdf.arc_to(s * 0.7083, s * 0.6667, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.7917, s * 0.7500)
            sdf.arc_to(s * 0.7083, s * 0.7500, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.2917, s * 0.8333)
            sdf.arc_to(s * 0.2917, s * 0.7500, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.2083, s * 0.6667)
            sdf.arc_to(s * 0.2917, s * 0.6667, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.3750, s * 0.1667)
            sdf.line_to(s * 0.6250, s * 0.1667)
            sdf.arc_to(s * 0.6250, s * 0.2500, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.7083, s * 0.3333)
            sdf.arc_to(s * 0.6250, s * 0.3333, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.3750, s * 0.4167)
            sdf.arc_to(s * 0.3750, s * 0.3333, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.2917, s * 0.2500)
            sdf.arc_to(s * 0.3750, s * 0.2500, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/align-vertical-distribute-end.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/align-vertical-distribute-end.svg via scripts/gen-icon.py.
    mod.draw.IconAlignVerticalDistributeEnd = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.2917, s * 0.5833)
            sdf.line_to(s * 0.7083, s * 0.5833)
            sdf.arc_to(s * 0.7083, s * 0.6667, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.7917, s * 0.7500)
            sdf.arc_to(s * 0.7083, s * 0.7500, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.2917, s * 0.8333)
            sdf.arc_to(s * 0.2917, s * 0.7500, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.2083, s * 0.6667)
            sdf.arc_to(s * 0.2917, s * 0.6667, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.3750, s * 0.1667)
            sdf.line_to(s * 0.6250, s * 0.1667)
            sdf.arc_to(s * 0.6250, s * 0.2500, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.7083, s * 0.3333)
            sdf.arc_to(s * 0.6250, s * 0.3333, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.3750, s * 0.4167)
            sdf.arc_to(s * 0.3750, s * 0.3333, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.2917, s * 0.2500)
            sdf.arc_to(s * 0.3750, s * 0.2500, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.0833, s * 0.8333)
            sdf.line_to(s * 0.9167, s * 0.8333)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.0833, s * 0.4167)
            sdf.line_to(s * 0.9167, s * 0.4167)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/align-vertical-distribute-start.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/align-vertical-distribute-start.svg via scripts/gen-icon.py.
    mod.draw.IconAlignVerticalDistributeStart = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.2917, s * 0.5833)
            sdf.line_to(s * 0.7083, s * 0.5833)
            sdf.arc_to(s * 0.7083, s * 0.6667, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.7917, s * 0.7500)
            sdf.arc_to(s * 0.7083, s * 0.7500, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.2917, s * 0.8333)
            sdf.arc_to(s * 0.2917, s * 0.7500, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.2083, s * 0.6667)
            sdf.arc_to(s * 0.2917, s * 0.6667, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.3750, s * 0.1667)
            sdf.line_to(s * 0.6250, s * 0.1667)
            sdf.arc_to(s * 0.6250, s * 0.2500, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.7083, s * 0.3333)
            sdf.arc_to(s * 0.6250, s * 0.3333, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.3750, s * 0.4167)
            sdf.arc_to(s * 0.3750, s * 0.3333, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.2917, s * 0.2500)
            sdf.arc_to(s * 0.3750, s * 0.2500, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.0833, s * 0.5833)
            sdf.line_to(s * 0.9167, s * 0.5833)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.0833, s * 0.1667)
            sdf.line_to(s * 0.9167, s * 0.1667)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/align-vertical-justify-center.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/align-vertical-justify-center.svg via scripts/gen-icon.py.
    mod.draw.IconAlignVerticalJustifyCenter = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.2917, s * 0.6667)
            sdf.line_to(s * 0.7083, s * 0.6667)
            sdf.arc_to(s * 0.7083, s * 0.7500, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.7917, s * 0.8333)
            sdf.arc_to(s * 0.7083, s * 0.8333, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.2917, s * 0.9167)
            sdf.arc_to(s * 0.2917, s * 0.8333, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.2083, s * 0.7500)
            sdf.arc_to(s * 0.2917, s * 0.7500, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.3750, s * 0.0833)
            sdf.line_to(s * 0.6250, s * 0.0833)
            sdf.arc_to(s * 0.6250, s * 0.1667, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.7083, s * 0.2500)
            sdf.arc_to(s * 0.6250, s * 0.2500, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.3750, s * 0.3333)
            sdf.arc_to(s * 0.3750, s * 0.2500, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.2917, s * 0.1667)
            sdf.arc_to(s * 0.3750, s * 0.1667, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.0833, s * 0.5000)
            sdf.line_to(s * 0.9167, s * 0.5000)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/align-vertical-justify-end.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/align-vertical-justify-end.svg via scripts/gen-icon.py.
    mod.draw.IconAlignVerticalJustifyEnd = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.2917, s * 0.5000)
            sdf.line_to(s * 0.7083, s * 0.5000)
            sdf.arc_to(s * 0.7083, s * 0.5833, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.7917, s * 0.6667)
            sdf.arc_to(s * 0.7083, s * 0.6667, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.2917, s * 0.7500)
            sdf.arc_to(s * 0.2917, s * 0.6667, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.2083, s * 0.5833)
            sdf.arc_to(s * 0.2917, s * 0.5833, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.3750, s * 0.0833)
            sdf.line_to(s * 0.6250, s * 0.0833)
            sdf.arc_to(s * 0.6250, s * 0.1667, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.7083, s * 0.2500)
            sdf.arc_to(s * 0.6250, s * 0.2500, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.3750, s * 0.3333)
            sdf.arc_to(s * 0.3750, s * 0.2500, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.2917, s * 0.1667)
            sdf.arc_to(s * 0.3750, s * 0.1667, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.0833, s * 0.9167)
            sdf.line_to(s * 0.9167, s * 0.9167)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/align-vertical-justify-start.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/align-vertical-justify-start.svg via scripts/gen-icon.py.
    mod.draw.IconAlignVerticalJustifyStart = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.2917, s * 0.6667)
            sdf.line_to(s * 0.7083, s * 0.6667)
            sdf.arc_to(s * 0.7083, s * 0.7500, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.7917, s * 0.8333)
            sdf.arc_to(s * 0.7083, s * 0.8333, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.2917, s * 0.9167)
            sdf.arc_to(s * 0.2917, s * 0.8333, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.2083, s * 0.7500)
            sdf.arc_to(s * 0.2917, s * 0.7500, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.3750, s * 0.2500)
            sdf.line_to(s * 0.6250, s * 0.2500)
            sdf.arc_to(s * 0.6250, s * 0.3333, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.7083, s * 0.4167)
            sdf.arc_to(s * 0.6250, s * 0.4167, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.3750, s * 0.5000)
            sdf.arc_to(s * 0.3750, s * 0.4167, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.2917, s * 0.3333)
            sdf.arc_to(s * 0.3750, s * 0.3333, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.0833, s * 0.0833)
            sdf.line_to(s * 0.9167, s * 0.0833)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/align-vertical-space-around.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/align-vertical-space-around.svg via scripts/gen-icon.py.
    mod.draw.IconAlignVerticalSpaceAround = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.3750, s * 0.3750)
            sdf.line_to(s * 0.6250, s * 0.3750)
            sdf.arc_to(s * 0.6250, s * 0.4583, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.7083, s * 0.5417)
            sdf.arc_to(s * 0.6250, s * 0.5417, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.3750, s * 0.6250)
            sdf.arc_to(s * 0.3750, s * 0.5417, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.2917, s * 0.4583)
            sdf.arc_to(s * 0.3750, s * 0.4583, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.9167, s * 0.8333)
            sdf.line_to(s * 0.0833, s * 0.8333)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.9167, s * 0.1667)
            sdf.line_to(s * 0.0833, s * 0.1667)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/align-vertical-space-between.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/align-vertical-space-between.svg via scripts/gen-icon.py.
    mod.draw.IconAlignVerticalSpaceBetween = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.2917, s * 0.6250)
            sdf.line_to(s * 0.7083, s * 0.6250)
            sdf.arc_to(s * 0.7083, s * 0.7083, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.7917, s * 0.7917)
            sdf.arc_to(s * 0.7083, s * 0.7917, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.2917, s * 0.8750)
            sdf.arc_to(s * 0.2917, s * 0.7917, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.2083, s * 0.7083)
            sdf.arc_to(s * 0.2917, s * 0.7083, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.3750, s * 0.1250)
            sdf.line_to(s * 0.6250, s * 0.1250)
            sdf.arc_to(s * 0.6250, s * 0.2083, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.7083, s * 0.2917)
            sdf.arc_to(s * 0.6250, s * 0.2917, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.3750, s * 0.3750)
            sdf.arc_to(s * 0.3750, s * 0.2917, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.2917, s * 0.2083)
            sdf.arc_to(s * 0.3750, s * 0.2083, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.0833, s * 0.8750)
            sdf.line_to(s * 0.9167, s * 0.8750)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.0833, s * 0.1250)
            sdf.line_to(s * 0.9167, s * 0.1250)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/arrow-down-a-z.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/arrow-down-a-z.svg via scripts/gen-icon.py.
    mod.draw.IconArrowDownAZ = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.1250, s * 0.6667)
            sdf.line_to(s * 0.2917, s * 0.8333)
            sdf.line_to(s * 0.4583, s * 0.6667)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2917, s * 0.8333)
            sdf.line_to(s * 0.2917, s * 0.1667)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.8333, s * 0.3333)
            sdf.line_to(s * 0.6250, s * 0.3333)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6250, s * 0.4167)
            sdf.line_to(s * 0.6250, s * 0.2708)
            sdf.arc_to(s * 0.7292, s * 0.2708, s * 0.1042, 3.1416, 6.2832)
            sdf.line_to(s * 0.8333, s * 0.4167)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6250, s * 0.5833)
            sdf.line_to(s * 0.8333, s * 0.5833)
            sdf.line_to(s * 0.6250, s * 0.8333)
            sdf.line_to(s * 0.8333, s * 0.8333)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/arrow-down-z-a.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/arrow-down-z-a.svg via scripts/gen-icon.py.
    mod.draw.IconArrowDownZA = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.1250, s * 0.6667)
            sdf.line_to(s * 0.2917, s * 0.8333)
            sdf.line_to(s * 0.4583, s * 0.6667)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2917, s * 0.1667)
            sdf.line_to(s * 0.2917, s * 0.8333)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6250, s * 0.1667)
            sdf.line_to(s * 0.8333, s * 0.1667)
            sdf.line_to(s * 0.6250, s * 0.4167)
            sdf.line_to(s * 0.8333, s * 0.4167)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6250, s * 0.8333)
            sdf.line_to(s * 0.6250, s * 0.6875)
            sdf.arc_to(s * 0.7292, s * 0.6875, s * 0.1042, 3.1416, 6.2832)
            sdf.line_to(s * 0.8333, s * 0.8333)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.8333, s * 0.7500)
            sdf.line_to(s * 0.6250, s * 0.7500)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/arrow-up-a-z.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/arrow-up-a-z.svg via scripts/gen-icon.py.
    mod.draw.IconArrowUpAZ = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.1250, s * 0.3333)
            sdf.line_to(s * 0.2917, s * 0.1667)
            sdf.line_to(s * 0.4583, s * 0.3333)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2917, s * 0.1667)
            sdf.line_to(s * 0.2917, s * 0.8333)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.8333, s * 0.3333)
            sdf.line_to(s * 0.6250, s * 0.3333)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6250, s * 0.4167)
            sdf.line_to(s * 0.6250, s * 0.2708)
            sdf.arc_to(s * 0.7292, s * 0.2708, s * 0.1042, 3.1416, 6.2832)
            sdf.line_to(s * 0.8333, s * 0.4167)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6250, s * 0.5833)
            sdf.line_to(s * 0.8333, s * 0.5833)
            sdf.line_to(s * 0.6250, s * 0.8333)
            sdf.line_to(s * 0.8333, s * 0.8333)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/between-horizontal-end.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/between-horizontal-end.svg via scripts/gen-icon.py.
    mod.draw.IconBetweenHorizontalEnd = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.1667, s * 0.1250)
            sdf.line_to(s * 0.6250, s * 0.1250)
            sdf.arc_to(s * 0.6250, s * 0.1667, s * 0.0417, -1.5708, 0.0000)
            sdf.line_to(s * 0.6667, s * 0.3750)
            sdf.arc_to(s * 0.6250, s * 0.3750, s * 0.0417, 0.0000, 1.5708)
            sdf.line_to(s * 0.1667, s * 0.4167)
            sdf.arc_to(s * 0.1667, s * 0.3750, s * 0.0417, 1.5708, 3.1416)
            sdf.line_to(s * 0.1250, s * 0.1667)
            sdf.arc_to(s * 0.1667, s * 0.1667, s * 0.0417, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.9167, s * 0.6250)
            sdf.line_to(s * 0.7917, s * 0.5000)
            sdf.line_to(s * 0.9167, s * 0.3750)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.1667, s * 0.5833)
            sdf.line_to(s * 0.6250, s * 0.5833)
            sdf.arc_to(s * 0.6250, s * 0.6250, s * 0.0417, -1.5708, 0.0000)
            sdf.line_to(s * 0.6667, s * 0.8333)
            sdf.arc_to(s * 0.6250, s * 0.8333, s * 0.0417, 0.0000, 1.5708)
            sdf.line_to(s * 0.1667, s * 0.8750)
            sdf.arc_to(s * 0.1667, s * 0.8333, s * 0.0417, 1.5708, 3.1416)
            sdf.line_to(s * 0.1250, s * 0.6250)
            sdf.arc_to(s * 0.1667, s * 0.6250, s * 0.0417, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/between-horizontal-start.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/between-horizontal-start.svg via scripts/gen-icon.py.
    mod.draw.IconBetweenHorizontalStart = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.3750, s * 0.1250)
            sdf.line_to(s * 0.8333, s * 0.1250)
            sdf.arc_to(s * 0.8333, s * 0.1667, s * 0.0417, -1.5708, 0.0000)
            sdf.line_to(s * 0.8750, s * 0.3750)
            sdf.arc_to(s * 0.8333, s * 0.3750, s * 0.0417, 0.0000, 1.5708)
            sdf.line_to(s * 0.3750, s * 0.4167)
            sdf.arc_to(s * 0.3750, s * 0.3750, s * 0.0417, 1.5708, 3.1416)
            sdf.line_to(s * 0.3333, s * 0.1667)
            sdf.arc_to(s * 0.3750, s * 0.1667, s * 0.0417, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.0833, s * 0.3750)
            sdf.line_to(s * 0.2083, s * 0.5000)
            sdf.line_to(s * 0.0833, s * 0.6250)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.3750, s * 0.5833)
            sdf.line_to(s * 0.8333, s * 0.5833)
            sdf.arc_to(s * 0.8333, s * 0.6250, s * 0.0417, -1.5708, 0.0000)
            sdf.line_to(s * 0.8750, s * 0.8333)
            sdf.arc_to(s * 0.8333, s * 0.8333, s * 0.0417, 0.0000, 1.5708)
            sdf.line_to(s * 0.3750, s * 0.8750)
            sdf.arc_to(s * 0.3750, s * 0.8333, s * 0.0417, 1.5708, 3.1416)
            sdf.line_to(s * 0.3333, s * 0.6250)
            sdf.arc_to(s * 0.3750, s * 0.6250, s * 0.0417, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/between-vertical-end.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/between-vertical-end.svg via scripts/gen-icon.py.
    mod.draw.IconBetweenVerticalEnd = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.1667, s * 0.1250)
            sdf.line_to(s * 0.3750, s * 0.1250)
            sdf.arc_to(s * 0.3750, s * 0.1667, s * 0.0417, -1.5708, 0.0000)
            sdf.line_to(s * 0.4167, s * 0.6250)
            sdf.arc_to(s * 0.3750, s * 0.6250, s * 0.0417, 0.0000, 1.5708)
            sdf.line_to(s * 0.1667, s * 0.6667)
            sdf.arc_to(s * 0.1667, s * 0.6250, s * 0.0417, 1.5708, 3.1416)
            sdf.line_to(s * 0.1250, s * 0.1667)
            sdf.arc_to(s * 0.1667, s * 0.1667, s * 0.0417, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.3750, s * 0.9167)
            sdf.line_to(s * 0.5000, s * 0.7917)
            sdf.line_to(s * 0.6250, s * 0.9167)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6250, s * 0.1250)
            sdf.line_to(s * 0.8333, s * 0.1250)
            sdf.arc_to(s * 0.8333, s * 0.1667, s * 0.0417, -1.5708, 0.0000)
            sdf.line_to(s * 0.8750, s * 0.6250)
            sdf.arc_to(s * 0.8333, s * 0.6250, s * 0.0417, 0.0000, 1.5708)
            sdf.line_to(s * 0.6250, s * 0.6667)
            sdf.arc_to(s * 0.6250, s * 0.6250, s * 0.0417, 1.5708, 3.1416)
            sdf.line_to(s * 0.5833, s * 0.1667)
            sdf.arc_to(s * 0.6250, s * 0.1667, s * 0.0417, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/between-vertical-start.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/between-vertical-start.svg via scripts/gen-icon.py.
    mod.draw.IconBetweenVerticalStart = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.1667, s * 0.3333)
            sdf.line_to(s * 0.3750, s * 0.3333)
            sdf.arc_to(s * 0.3750, s * 0.3750, s * 0.0417, -1.5708, 0.0000)
            sdf.line_to(s * 0.4167, s * 0.8333)
            sdf.arc_to(s * 0.3750, s * 0.8333, s * 0.0417, 0.0000, 1.5708)
            sdf.line_to(s * 0.1667, s * 0.8750)
            sdf.arc_to(s * 0.1667, s * 0.8333, s * 0.0417, 1.5708, 3.1416)
            sdf.line_to(s * 0.1250, s * 0.3750)
            sdf.arc_to(s * 0.1667, s * 0.3750, s * 0.0417, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6250, s * 0.0833)
            sdf.line_to(s * 0.5000, s * 0.2083)
            sdf.line_to(s * 0.3750, s * 0.0833)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6250, s * 0.3333)
            sdf.line_to(s * 0.8333, s * 0.3333)
            sdf.arc_to(s * 0.8333, s * 0.3750, s * 0.0417, -1.5708, 0.0000)
            sdf.line_to(s * 0.8750, s * 0.8333)
            sdf.arc_to(s * 0.8333, s * 0.8333, s * 0.0417, 0.0000, 1.5708)
            sdf.line_to(s * 0.6250, s * 0.8750)
            sdf.arc_to(s * 0.6250, s * 0.8333, s * 0.0417, 1.5708, 3.1416)
            sdf.line_to(s * 0.5833, s * 0.3750)
            sdf.arc_to(s * 0.6250, s * 0.3750, s * 0.0417, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/cable.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/cable.svg via scripts/gen-icon.py.
    mod.draw.IconCable = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.7083, s * 0.7917)
            sdf.arc_to(s * 0.7083, s * 0.7500, s * 0.0417, 1.5708, 3.1416)
            sdf.line_to(s * 0.6667, s * 0.6667)
            sdf.arc_to(s * 0.7500, s * 0.6667, s * 0.0833, 3.1416, 4.7124)
            sdf.line_to(s * 0.8333, s * 0.5833)
            sdf.arc_to(s * 0.8333, s * 0.6667, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.9167, s * 0.7500)
            sdf.arc_to(s * 0.8750, s * 0.7500, s * 0.0417, 0.0000, 1.5708)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.7083, s * 0.8750)
            sdf.line_to(s * 0.7083, s * 0.7917)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.7917, s * 0.5833)
            sdf.line_to(s * 0.7917, s * 0.2708)
            sdf.arc_to(s * 0.6458, s * 0.2708, s * 0.1458, 0.0000, -3.1416)
            sdf.line_to(s * 0.5000, s * 0.7292)
            sdf.arc_to(s * 0.3542, s * 0.7292, s * 0.1458, 0.0000, 3.1416)
            sdf.line_to(s * 0.2083, s * 0.4167)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.8750, s * 0.8750)
            sdf.line_to(s * 0.8750, s * 0.7917)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.1250, s * 0.2083)
            sdf.line_to(s * 0.1250, s * 0.1250)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.1667, s * 0.4167)
            sdf.arc_to(s * 0.1667, s * 0.3333, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.0833, s * 0.2500)
            sdf.arc_to(s * 0.1250, s * 0.2500, s * 0.0417, 3.1416, 4.7124)
            sdf.line_to(s * 0.2917, s * 0.2083)
            sdf.arc_to(s * 0.2917, s * 0.2500, s * 0.0417, -1.5708, 0.0000)
            sdf.line_to(s * 0.3333, s * 0.3333)
            sdf.arc_to(s * 0.2500, s * 0.3333, s * 0.0833, 0.0000, 1.5708)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2917, s * 0.2083)
            sdf.line_to(s * 0.2917, s * 0.1250)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/chevrons-down-up.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/chevrons-down-up.svg via scripts/gen-icon.py.
    mod.draw.IconChevronsDownUp = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.2917, s * 0.8333)
            sdf.line_to(s * 0.5000, s * 0.6250)
            sdf.line_to(s * 0.7083, s * 0.8333)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2917, s * 0.1667)
            sdf.line_to(s * 0.5000, s * 0.3750)
            sdf.line_to(s * 0.7083, s * 0.1667)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/chevrons-left-right-ellipsis.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/chevrons-left-right-ellipsis.svg via scripts/gen-icon.py.
    mod.draw.IconChevronsLeftRightEllipsis = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.5000, s * 0.5000)
            sdf.line_to(s * 0.5004, s * 0.5000)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6667, s * 0.5000)
            sdf.line_to(s * 0.6671, s * 0.5000)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.7083, s * 0.2917)
            sdf.line_to(s * 0.9167, s * 0.5000)
            sdf.line_to(s * 0.7083, s * 0.7083)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2917, s * 0.2917)
            sdf.line_to(s * 0.0833, s * 0.5000)
            sdf.line_to(s * 0.2917, s * 0.7083)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.3333, s * 0.5000)
            sdf.line_to(s * 0.3338, s * 0.5000)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/chevrons-up.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/chevrons-up.svg via scripts/gen-icon.py.
    mod.draw.IconChevronsUp = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.7083, s * 0.4583)
            sdf.line_to(s * 0.5000, s * 0.2500)
            sdf.line_to(s * 0.2917, s * 0.4583)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.7083, s * 0.7500)
            sdf.line_to(s * 0.5000, s * 0.5417)
            sdf.line_to(s * 0.2917, s * 0.7500)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/chevrons-up-down.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/chevrons-up-down.svg via scripts/gen-icon.py.
    mod.draw.IconChevronsUpDown = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.2917, s * 0.6250)
            sdf.line_to(s * 0.5000, s * 0.8333)
            sdf.line_to(s * 0.7083, s * 0.6250)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2917, s * 0.3750)
            sdf.line_to(s * 0.5000, s * 0.1667)
            sdf.line_to(s * 0.7083, s * 0.3750)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/circle-x.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/circle-x.svg via scripts/gen-icon.py.
    mod.draw.IconCircleX = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.9167, s * 0.5000)
            sdf.arc_to(s * 0.5000, s * 0.5000, s * 0.4167, 0.0000, 3.1416)
            sdf.arc_to(s * 0.5000, s * 0.5000, s * 0.4167, 3.1416, 6.2832)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6250, s * 0.3750)
            sdf.line_to(s * 0.3750, s * 0.6250)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.3750, s * 0.3750)
            sdf.line_to(s * 0.6250, s * 0.6250)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/eye.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/eye.svg via scripts/gen-icon.py.
    mod.draw.IconEye = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.0859, s * 0.5145)
            sdf.arc_to(s * 0.1250, s * 0.5000, s * 0.0417, 2.7862, 3.4970)
            sdf.arc_to(s * 0.5000, s * 0.6563, s * 0.4479, -2.7504, -0.3912)
            sdf.arc_to(s * 0.8750, s * 0.5000, s * 0.0417, -0.3554, 0.3554)
            sdf.arc_to(s * 0.5000, s * 0.3437, s * 0.4479, 0.3912, 2.7504)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6250, s * 0.5000)
            sdf.arc_to(s * 0.5000, s * 0.5000, s * 0.1250, 0.0000, 3.1416)
            sdf.arc_to(s * 0.5000, s * 0.5000, s * 0.1250, 3.1416, 6.2832)
            sdf.close_path()
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/eye-off.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/eye-off.svg via scripts/gen-icon.py.
    mod.draw.IconEyeOff = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.4472, s * 0.2115)
            sdf.arc_to(s * 0.5002, s * 0.6560, s * 0.4477, -1.6894, -0.3909)
            sdf.arc_to(s * 0.8750, s * 0.5000, s * 0.0417, -0.3554, 0.3554)
            sdf.arc_to(s * 0.5001, s * 0.3437, s * 0.4478, 0.3912, 0.6598)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.5868, s * 0.5899)
            sdf.arc_to(s * 0.5000, s * 0.5000, s * 0.1250, 0.8028, 3.9096)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.7283, s * 0.7291)
            sdf.arc_to(s * 0.5000, s * 0.3437, s * 0.4479, 1.0360, 2.7505)
            sdf.arc_to(s * 0.1250, s * 0.5000, s * 0.0417, 2.7862, 3.4970)
            sdf.arc_to(s * 0.5000, s * 0.6563, s * 0.4479, -2.7505, -2.1070)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.0833, s * 0.0833)
            sdf.line_to(s * 0.9167, s * 0.9167)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/frame.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/frame.svg via scripts/gen-icon.py.
    mod.draw.IconFrame = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.9167, s * 0.2500)
            sdf.line_to(s * 0.0833, s * 0.2500)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.9167, s * 0.7500)
            sdf.line_to(s * 0.0833, s * 0.7500)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2500, s * 0.0833)
            sdf.line_to(s * 0.2500, s * 0.9167)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.7500, s * 0.0833)
            sdf.line_to(s * 0.7500, s * 0.9167)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/funnel.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/funnel.svg via scripts/gen-icon.py.
    mod.draw.IconFunnel = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.4167, s * 0.8333)
            sdf.arc_to(s * 0.4583, s * 0.8334, s * 0.0417, -3.1411, -4.2490)
            sdf.line_to(s * 0.5230, s * 0.9123)
            sdf.arc_to(s * 0.5417, s * 0.8750, s * 0.0417, 2.0342, -0.0005)
            sdf.line_to(s * 0.5833, s * 0.5833)
            sdf.arc_to(s * 0.6667, s * 0.5834, s * 0.0833, -3.1411, -2.4061)
            sdf.line_to(s * 0.9058, s * 0.1946)
            sdf.arc_to(s * 0.8749, s * 0.1667, s * 0.0417, 0.7342, -1.5684)
            sdf.line_to(s * 0.1250, s * 0.1250)
            sdf.arc_to(s * 0.1250, s * 0.1667, s * 0.0417, -1.5712, -3.8758)
            sdf.line_to(s * 0.3951, s * 0.5275)
            sdf.arc_to(s * 0.3333, s * 0.5834, s * 0.0833, -0.7355, -0.0005)
            sdf.close_path()
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/funnel-x.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/funnel-x.svg via scripts/gen-icon.py.
    mod.draw.IconFunnelX = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.5221, s * 0.1250)
            sdf.line_to(s * 0.1250, s * 0.1250)
            sdf.arc_to(s * 0.1250, s * 0.1667, s * 0.0417, -1.5712, -3.8758)
            sdf.line_to(s * 0.3951, s * 0.5275)
            sdf.arc_to(s * 0.3333, s * 0.5834, s * 0.0833, -0.7355, -0.0005)
            sdf.line_to(s * 0.4167, s * 0.8333)
            sdf.arc_to(s * 0.4583, s * 0.8334, s * 0.0417, -3.1411, -4.2490)
            sdf.line_to(s * 0.5230, s * 0.9123)
            sdf.arc_to(s * 0.5417, s * 0.8750, s * 0.0417, 2.0342, -0.0005)
            sdf.line_to(s * 0.5833, s * 0.5833)
            sdf.arc_to(s * 0.6667, s * 0.5834, s * 0.0833, -3.1411, -2.4061)
            sdf.line_to(s * 0.6227, s * 0.5078)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6875, s * 0.1458)
            sdf.line_to(s * 0.8958, s * 0.3542)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.8958, s * 0.1458)
            sdf.line_to(s * 0.6875, s * 0.3542)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/grip-vertical.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/grip-vertical.svg via scripts/gen-icon.py.
    mod.draw.IconGripVertical = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.4167, s * 0.5000)
            sdf.arc_to(s * 0.3750, s * 0.5000, s * 0.0417, 0.0000, 3.1416)
            sdf.arc_to(s * 0.3750, s * 0.5000, s * 0.0417, 3.1416, 6.2832)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.4167, s * 0.2083)
            sdf.arc_to(s * 0.3750, s * 0.2083, s * 0.0417, 0.0000, 3.1416)
            sdf.arc_to(s * 0.3750, s * 0.2083, s * 0.0417, 3.1416, 6.2832)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.4167, s * 0.7917)
            sdf.arc_to(s * 0.3750, s * 0.7917, s * 0.0417, 0.0000, 3.1416)
            sdf.arc_to(s * 0.3750, s * 0.7917, s * 0.0417, 3.1416, 6.2832)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6667, s * 0.5000)
            sdf.arc_to(s * 0.6250, s * 0.5000, s * 0.0417, 0.0000, 3.1416)
            sdf.arc_to(s * 0.6250, s * 0.5000, s * 0.0417, 3.1416, 6.2832)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6667, s * 0.2083)
            sdf.arc_to(s * 0.6250, s * 0.2083, s * 0.0417, 0.0000, 3.1416)
            sdf.arc_to(s * 0.6250, s * 0.2083, s * 0.0417, 3.1416, 6.2832)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6667, s * 0.7917)
            sdf.arc_to(s * 0.6250, s * 0.7917, s * 0.0417, 0.0000, 3.1416)
            sdf.arc_to(s * 0.6250, s * 0.7917, s * 0.0417, 3.1416, 6.2832)
            sdf.close_path()
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/info.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/info.svg via scripts/gen-icon.py.
    mod.draw.IconInfo = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.9167, s * 0.5000)
            sdf.arc_to(s * 0.5000, s * 0.5000, s * 0.4167, 0.0000, 3.1416)
            sdf.arc_to(s * 0.5000, s * 0.5000, s * 0.4167, 3.1416, 6.2832)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.5000, s * 0.6667)
            sdf.line_to(s * 0.5000, s * 0.5000)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.5000, s * 0.3333)
            sdf.line_to(s * 0.5004, s * 0.3333)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/mouse-pointer-2.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/mouse-pointer-2.svg via scripts/gen-icon.py.
    mod.draw.IconMousePointer2 = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.1682, s * 0.1953)
            sdf.arc_to(s * 0.1871, s * 0.1871, s * 0.0206, 2.7327, 5.1213)
            sdf.line_to(s * 0.8620, s * 0.4390)
            sdf.arc_to(s * 0.8541, s * 0.4583, s * 0.0208, -1.1839, 1.3168)
            sdf.line_to(s * 0.6042, s * 0.5443)
            sdf.arc_to(s * 0.6250, s * 0.6250, s * 0.0833, -1.8224, -2.8879)
            sdf.line_to(s * 0.4785, s * 0.8594)
            sdf.arc_to(s * 0.4583, s * 0.8541, s * 0.0208, 0.2540, 2.7547)
            sdf.close_path()
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/package-check.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/package-check.svg via scripts/gen-icon.py.
    mod.draw.IconPackageCheck = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.5000, s * 0.9167)
            sdf.line_to(s * 0.5000, s * 0.5000)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6667, s * 0.7083)
            sdf.line_to(s * 0.7500, s * 0.7917)
            sdf.line_to(s * 0.9167, s * 0.6250)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.8750, s * 0.4636)
            sdf.line_to(s * 0.8750, s * 0.3333)
            sdf.arc_to(s * 0.7917, s * 0.3334, s * 0.0833, -0.0010, -1.0472)
            sdf.line_to(s * 0.5417, s * 0.0946)
            sdf.arc_to(s * 0.5000, s * 0.1668, s * 0.0833, -1.0472, -2.0944)
            sdf.line_to(s * 0.1667, s * 0.2613)
            sdf.arc_to(s * 0.2083, s * 0.3334, s * 0.0833, -2.0944, -3.1406)
            sdf.line_to(s * 0.1250, s * 0.6667)
            sdf.arc_to(s * 0.2083, s * 0.6665, s * 0.0833, 3.1401, 2.0944)
            sdf.line_to(s * 0.4583, s * 0.9054)
            sdf.arc_to(s * 0.5000, s * 0.8332, s * 0.0833, 2.0949, 1.0477)
            sdf.line_to(s * 0.5967, s * 0.8740)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.1371, s * 0.2917)
            sdf.line_to(s * 0.5000, s * 0.5000)
            sdf.line_to(s * 0.8629, s * 0.2917)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.3125, s * 0.1779)
            sdf.line_to(s * 0.6874, s * 0.3924)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/package-open.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/package-open.svg via scripts/gen-icon.py.
    mod.draw.IconPackageOpen = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.5000, s * 0.9167)
            sdf.line_to(s * 0.5000, s * 0.5417)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6321, s * 0.0921)
            sdf.arc_to(s * 0.6660, s * 0.1528, s * 0.0696, -2.0806, -1.0610)
            sdf.line_to(s * 0.8750, s * 0.1904)
            sdf.arc_to(s * 0.8354, s * 0.2604, s * 0.0804, -1.0561, 1.0561)
            sdf.line_to(s * 0.3675, s * 0.6163)
            sdf.arc_to(s * 0.3333, s * 0.5564, s * 0.0690, 1.0524, 2.0892)
            sdf.line_to(s * 0.1250, s * 0.5179)
            sdf.arc_to(s * 0.1646, s * 0.4479, s * 0.0804, 2.0854, 4.1977)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.8333, s * 0.5417)
            sdf.line_to(s * 0.8333, s * 0.7029)
            sdf.arc_to(s * 0.7475, s * 0.7030, s * 0.0858, -0.0010, 1.0915)
            sdf.line_to(s * 0.5371, s * 0.9075)
            sdf.arc_to(s * 0.5000, s * 0.8361, s * 0.0804, 1.0915, 2.0501)
            sdf.line_to(s * 0.2129, s * 0.7792)
            sdf.arc_to(s * 0.2525, s * 0.7030, s * 0.0858, 2.0501, 3.1426)
            sdf.line_to(s * 0.1667, s * 0.5417)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.8750, s * 0.5179)
            sdf.arc_to(s * 0.8354, s * 0.4479, s * 0.0804, 1.0561, -1.0561)
            sdf.line_to(s * 0.3679, s * 0.0917)
            sdf.arc_to(s * 0.3340, s * 0.1510, s * 0.0683, -1.0507, -2.0909)
            sdf.line_to(s * 0.1250, s * 0.1904)
            sdf.arc_to(s * 0.1646, s * 0.2604, s * 0.0804, -2.0854, -4.1977)
            sdf.line_to(s * 0.6325, s * 0.6163)
            sdf.arc_to(s * 0.6665, s * 0.5571, s * 0.0682, 2.0923, 1.0493)
            sdf.close_path()
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/panel-top.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/panel-top.svg via scripts/gen-icon.py.
    mod.draw.IconPanelTop = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.2083, s * 0.1250)
            sdf.line_to(s * 0.7917, s * 0.1250)
            sdf.arc_to(s * 0.7917, s * 0.2083, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.8750, s * 0.7917)
            sdf.arc_to(s * 0.7917, s * 0.7917, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.2083, s * 0.8750)
            sdf.arc_to(s * 0.2083, s * 0.7917, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.1250, s * 0.2083)
            sdf.arc_to(s * 0.2083, s * 0.2083, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.1250, s * 0.3750)
            sdf.line_to(s * 0.8750, s * 0.3750)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/radar.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/radar.svg via scripts/gen-icon.py.
    mod.draw.IconRadar = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.7946, s * 0.2054)
            sdf.arc_to(s * 0.4998, s * 0.4999, s * 0.4167, -0.7849, -2.0950)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.1667, s * 0.2500)
            sdf.line_to(s * 0.1671, s * 0.2500)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.0954, s * 0.4008)
            sdf.arc_to(s * 0.5001, s * 0.5002, s * 0.4167, -2.9008, -6.6573)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6767, s * 0.3233)
            sdf.arc_to(s * 0.4994, s * 0.4996, s * 0.2500, -0.7825, -4.0362)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.5000, s * 0.7500)
            sdf.line_to(s * 0.5004, s * 0.7500)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.7496, s * 0.4858)
            sdf.arc_to(s * 0.5000, s * 0.5001, s * 0.2500, -0.0571, 0.8913)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.5833, s * 0.5000)
            sdf.arc_to(s * 0.5000, s * 0.5000, s * 0.0833, 0.0000, 3.1416)
            sdf.arc_to(s * 0.5000, s * 0.5000, s * 0.0833, 3.1416, 6.2832)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.5588, s * 0.4413)
            sdf.line_to(s * 0.7946, s * 0.2054)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/save.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/save.svg via scripts/gen-icon.py.
    mod.draw.IconSave = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.6333, s * 0.1250)
            sdf.arc_to(s * 0.6321, s * 0.2083, s * 0.0833, -1.5566, -0.7753)
            sdf.line_to(s * 0.8500, s * 0.3083)
            sdf.arc_to(s * 0.7917, s * 0.3679, s * 0.0833, -0.7955, -0.0142)
            sdf.line_to(s * 0.8750, s * 0.7917)
            sdf.arc_to(s * 0.7917, s * 0.7917, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.2083, s * 0.8750)
            sdf.arc_to(s * 0.2083, s * 0.7917, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.1250, s * 0.2083)
            sdf.arc_to(s * 0.2083, s * 0.2083, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.7083, s * 0.8750)
            sdf.line_to(s * 0.7083, s * 0.5833)
            sdf.arc_to(s * 0.6667, s * 0.5833, s * 0.0417, 0.0000, -1.5708)
            sdf.line_to(s * 0.3333, s * 0.5417)
            sdf.arc_to(s * 0.3333, s * 0.5833, s * 0.0417, -1.5708, -3.1416)
            sdf.line_to(s * 0.2917, s * 0.8750)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2917, s * 0.1250)
            sdf.line_to(s * 0.2917, s * 0.2917)
            sdf.arc_to(s * 0.3333, s * 0.2917, s * 0.0417, 3.1416, 1.5708)
            sdf.line_to(s * 0.6250, s * 0.3333)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/save-check.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/save-check.svg via scripts/gen-icon.py.
    mod.draw.IconSaveCheck = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.5208, s * 0.8750)
            sdf.line_to(s * 0.2083, s * 0.8750)
            sdf.arc_to(s * 0.2083, s * 0.7917, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.1250, s * 0.2083)
            sdf.arc_to(s * 0.2083, s * 0.2083, s * 0.0833, 3.1416, 4.7124)
            sdf.line_to(s * 0.6333, s * 0.1250)
            sdf.arc_to(s * 0.6321, s * 0.2083, s * 0.0833, -1.5566, -0.7753)
            sdf.line_to(s * 0.8500, s * 0.3083)
            sdf.arc_to(s * 0.7917, s * 0.3679, s * 0.0833, -0.7955, -0.0142)
            sdf.line_to(s * 0.8750, s * 0.5479)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6667, s * 0.7917)
            sdf.line_to(s * 0.7500, s * 0.8750)
            sdf.line_to(s * 0.9167, s * 0.7083)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.7083, s * 0.6304)
            sdf.line_to(s * 0.7083, s * 0.5833)
            sdf.arc_to(s * 0.6667, s * 0.5833, s * 0.0417, 0.0000, -1.5708)
            sdf.line_to(s * 0.3333, s * 0.5417)
            sdf.arc_to(s * 0.3333, s * 0.5833, s * 0.0417, -1.5708, -3.1416)
            sdf.line_to(s * 0.2917, s * 0.8750)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2917, s * 0.1250)
            sdf.line_to(s * 0.2917, s * 0.2917)
            sdf.arc_to(s * 0.3333, s * 0.2917, s * 0.0417, 3.1416, 1.5708)
            sdf.line_to(s * 0.6250, s * 0.3333)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/save-pen.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/save-pen.svg via scripts/gen-icon.py.
    mod.draw.IconSavePen = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.5554, s * 0.5417)
            sdf.line_to(s * 0.3333, s * 0.5417)
            sdf.arc_to(s * 0.3333, s * 0.5833, s * 0.0417, -1.5708, -3.1416)
            sdf.line_to(s * 0.2917, s * 0.8750)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.5985, s * 0.7348)
            sdf.arc_to(s * 0.6574, s * 0.7937, s * 0.0833, -2.3559, -2.8575)
            sdf.line_to(s * 0.5425, s * 0.8899)
            sdf.arc_to(s * 0.5625, s * 0.8958, s * 0.0208, -2.8578, -4.9962)
            sdf.line_to(s * 0.6879, s * 0.8809)
            sdf.arc_to(s * 0.6646, s * 0.8009, s * 0.0833, 1.2867, 0.7851)
            sdf.line_to(s * 0.8907, s * 0.6928)
            sdf.arc_to(s * 0.8281, s * 0.6302, s * 0.0885, 0.7854, -2.3562)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2917, s * 0.1250)
            sdf.line_to(s * 0.2917, s * 0.2917)
            sdf.arc_to(s * 0.3333, s * 0.2917, s * 0.0417, 3.1416, 1.5708)
            sdf.line_to(s * 0.6250, s * 0.3333)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.3750, s * 0.8750)
            sdf.line_to(s * 0.2083, s * 0.8750)
            sdf.arc_to(s * 0.2083, s * 0.7917, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.1250, s * 0.2083)
            sdf.arc_to(s * 0.2083, s * 0.2083, s * 0.0833, 3.1416, 4.7124)
            sdf.line_to(s * 0.6333, s * 0.1250)
            sdf.arc_to(s * 0.6321, s * 0.2083, s * 0.0833, -1.5566, -0.7753)
            sdf.line_to(s * 0.8500, s * 0.3083)
            sdf.arc_to(s * 0.7917, s * 0.3679, s * 0.0833, -0.7955, -0.0142)
            sdf.line_to(s * 0.8750, s * 0.3792)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/scan.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/scan.svg via scripts/gen-icon.py.
    mod.draw.IconScan = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.1250, s * 0.2917)
            sdf.line_to(s * 0.1250, s * 0.2083)
            sdf.arc_to(s * 0.2083, s * 0.2083, s * 0.0833, 3.1416, 4.7124)
            sdf.line_to(s * 0.2917, s * 0.1250)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.7083, s * 0.1250)
            sdf.line_to(s * 0.7917, s * 0.1250)
            sdf.arc_to(s * 0.7917, s * 0.2083, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.8750, s * 0.2917)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.8750, s * 0.7083)
            sdf.line_to(s * 0.8750, s * 0.7917)
            sdf.arc_to(s * 0.7917, s * 0.7917, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.7083, s * 0.8750)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2917, s * 0.8750)
            sdf.line_to(s * 0.2083, s * 0.8750)
            sdf.arc_to(s * 0.2083, s * 0.7917, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.1250, s * 0.7083)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/sliders-horizontal.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/sliders-horizontal.svg via scripts/gen-icon.py.
    mod.draw.IconSlidersHorizontal = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.4167, s * 0.2083)
            sdf.line_to(s * 0.1250, s * 0.2083)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.5000, s * 0.7917)
            sdf.line_to(s * 0.1250, s * 0.7917)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.5833, s * 0.1250)
            sdf.line_to(s * 0.5833, s * 0.2917)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6667, s * 0.7083)
            sdf.line_to(s * 0.6667, s * 0.8750)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.8750, s * 0.5000)
            sdf.line_to(s * 0.5000, s * 0.5000)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.8750, s * 0.7917)
            sdf.line_to(s * 0.6667, s * 0.7917)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.8750, s * 0.2083)
            sdf.line_to(s * 0.5833, s * 0.2083)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.3333, s * 0.4167)
            sdf.line_to(s * 0.3333, s * 0.5833)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.3333, s * 0.5000)
            sdf.line_to(s * 0.1250, s * 0.5000)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/square.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/square.svg via scripts/gen-icon.py.
    mod.draw.IconSquare = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.2083, s * 0.1250)
            sdf.line_to(s * 0.7917, s * 0.1250)
            sdf.arc_to(s * 0.7917, s * 0.2083, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.8750, s * 0.7917)
            sdf.arc_to(s * 0.7917, s * 0.7917, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.2083, s * 0.8750)
            sdf.arc_to(s * 0.2083, s * 0.7917, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.1250, s * 0.2083)
            sdf.arc_to(s * 0.2083, s * 0.2083, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/square-dashed-top-solid.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/square-dashed-top-solid.svg via scripts/gen-icon.py.
    mod.draw.IconSquareDashedTopSolid = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.5833, s * 0.8750)
            sdf.line_to(s * 0.6250, s * 0.8750)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.8750, s * 0.5833)
            sdf.line_to(s * 0.8750, s * 0.6250)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.8750, s * 0.7917)
            sdf.arc_to(s * 0.7917, s * 0.7917, s * 0.0833, 0.0000, 1.5708)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.8750, s * 0.3750)
            sdf.line_to(s * 0.8750, s * 0.4167)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.1250, s * 0.5833)
            sdf.line_to(s * 0.1250, s * 0.6250)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.1250, s * 0.2083)
            sdf.arc_to(s * 0.2083, s * 0.2083, s * 0.0833, 3.1416, 4.7124)
            sdf.line_to(s * 0.7917, s * 0.1250)
            sdf.arc_to(s * 0.7917, s * 0.2083, s * 0.0833, -1.5708, 0.0000)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.1250, s * 0.3750)
            sdf.line_to(s * 0.1250, s * 0.4167)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2083, s * 0.8750)
            sdf.arc_to(s * 0.2083, s * 0.7917, s * 0.0833, 1.5708, 3.1416)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.3750, s * 0.8750)
            sdf.line_to(s * 0.4167, s * 0.8750)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/square-menu.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/square-menu.svg via scripts/gen-icon.py.
    mod.draw.IconSquareMenu = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.2083, s * 0.1250)
            sdf.line_to(s * 0.7917, s * 0.1250)
            sdf.arc_to(s * 0.7917, s * 0.2083, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.8750, s * 0.7917)
            sdf.arc_to(s * 0.7917, s * 0.7917, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.2083, s * 0.8750)
            sdf.arc_to(s * 0.2083, s * 0.7917, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.1250, s * 0.2083)
            sdf.arc_to(s * 0.2083, s * 0.2083, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2917, s * 0.3333)
            sdf.line_to(s * 0.7083, s * 0.3333)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2917, s * 0.5000)
            sdf.line_to(s * 0.7083, s * 0.5000)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2917, s * 0.6667)
            sdf.line_to(s * 0.7083, s * 0.6667)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/squircle.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/squircle.svg via scripts/gen-icon.py.
    mod.draw.IconSquircle = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.5000, s * 0.1250)
            sdf.line_to(s * 0.5704, s * 0.1266)
            sdf.line_to(s * 0.6319, s * 0.1319)
            sdf.line_to(s * 0.6852, s * 0.1414)
            sdf.line_to(s * 0.7306, s * 0.1556)
            sdf.line_to(s * 0.7687, s * 0.1749)
            sdf.line_to(s * 0.8000, s * 0.2000)
            sdf.line_to(s * 0.8251, s * 0.2313)
            sdf.line_to(s * 0.8444, s * 0.2694)
            sdf.line_to(s * 0.8586, s * 0.3148)
            sdf.line_to(s * 0.8681, s * 0.3681)
            sdf.line_to(s * 0.8734, s * 0.4296)
            sdf.line_to(s * 0.8750, s * 0.5000)
            sdf.line_to(s * 0.8734, s * 0.5704)
            sdf.line_to(s * 0.8681, s * 0.6319)
            sdf.line_to(s * 0.8586, s * 0.6852)
            sdf.line_to(s * 0.8444, s * 0.7306)
            sdf.line_to(s * 0.8251, s * 0.7687)
            sdf.line_to(s * 0.8000, s * 0.8000)
            sdf.line_to(s * 0.7687, s * 0.8251)
            sdf.line_to(s * 0.7306, s * 0.8444)
            sdf.line_to(s * 0.6852, s * 0.8586)
            sdf.line_to(s * 0.6319, s * 0.8681)
            sdf.line_to(s * 0.5704, s * 0.8734)
            sdf.line_to(s * 0.5000, s * 0.8750)
            sdf.line_to(s * 0.4296, s * 0.8734)
            sdf.line_to(s * 0.3681, s * 0.8681)
            sdf.line_to(s * 0.3148, s * 0.8586)
            sdf.line_to(s * 0.2694, s * 0.8444)
            sdf.line_to(s * 0.2313, s * 0.8251)
            sdf.line_to(s * 0.2000, s * 0.8000)
            sdf.line_to(s * 0.1749, s * 0.7687)
            sdf.line_to(s * 0.1556, s * 0.7306)
            sdf.line_to(s * 0.1414, s * 0.6852)
            sdf.line_to(s * 0.1319, s * 0.6319)
            sdf.line_to(s * 0.1266, s * 0.5704)
            sdf.line_to(s * 0.1250, s * 0.5000)
            sdf.line_to(s * 0.1266, s * 0.4296)
            sdf.line_to(s * 0.1319, s * 0.3681)
            sdf.line_to(s * 0.1414, s * 0.3148)
            sdf.line_to(s * 0.1556, s * 0.2694)
            sdf.line_to(s * 0.1749, s * 0.2313)
            sdf.line_to(s * 0.2000, s * 0.2000)
            sdf.line_to(s * 0.2313, s * 0.1749)
            sdf.line_to(s * 0.2694, s * 0.1556)
            sdf.line_to(s * 0.3148, s * 0.1414)
            sdf.line_to(s * 0.3681, s * 0.1319)
            sdf.line_to(s * 0.4296, s * 0.1266)
            sdf.line_to(s * 0.5000, s * 0.1250)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/squircle-dashed.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/squircle-dashed.svg via scripts/gen-icon.py.
    mod.draw.IconSquircleDashed = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.5738, s * 0.1268)
            sdf.arc_to(s * 0.5000, s * 1.5415, s * 1.4167, -1.5187, -1.6229)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.5738, s * 0.8732)
            sdf.arc_to(s * 0.4996, s * -0.4998, s * 1.3750, 1.5168, 1.6242)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.8408, s * 0.7392)
            sdf.line_to(s * 0.8353, s * 0.7508)
            sdf.line_to(s * 0.8293, s * 0.7619)
            sdf.line_to(s * 0.8227, s * 0.7723)
            sdf.line_to(s * 0.8157, s * 0.7821)
            sdf.line_to(s * 0.8081, s * 0.7913)
            sdf.line_to(s * 0.8000, s * 0.8000)
            sdf.line_to(s * 0.7913, s * 0.8081)
            sdf.line_to(s * 0.7821, s * 0.8157)
            sdf.line_to(s * 0.7723, s * 0.8227)
            sdf.line_to(s * 0.7619, s * 0.8293)
            sdf.line_to(s * 0.7509, s * 0.8353)
            sdf.line_to(s * 0.7392, s * 0.8408)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.8408, s * 0.2608)
            sdf.line_to(s * 0.8353, s * 0.2491)
            sdf.line_to(s * 0.8292, s * 0.2381)
            sdf.line_to(s * 0.8227, s * 0.2277)
            sdf.line_to(s * 0.8157, s * 0.2179)
            sdf.line_to(s * 0.8081, s * 0.2087)
            sdf.line_to(s * 0.8000, s * 0.2000)
            sdf.line_to(s * 0.7913, s * 0.1919)
            sdf.line_to(s * 0.7821, s * 0.1843)
            sdf.line_to(s * 0.7723, s * 0.1773)
            sdf.line_to(s * 0.7618, s * 0.1708)
            sdf.line_to(s * 0.7508, s * 0.1647)
            sdf.line_to(s * 0.7392, s * 0.1592)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.8732, s * 0.4263)
            sdf.arc_to(s * -0.4998, s * 0.5000, s * 1.3750, -0.0537, 0.0537)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.1268, s * 0.4263)
            sdf.arc_to(s * 1.5416, s * 0.4996, s * 1.4167, -3.0898, -3.1940)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2608, s * 0.8408)
            sdf.line_to(s * 0.2492, s * 0.8353)
            sdf.line_to(s * 0.2381, s * 0.8292)
            sdf.line_to(s * 0.2277, s * 0.8227)
            sdf.line_to(s * 0.2179, s * 0.8157)
            sdf.line_to(s * 0.2087, s * 0.8081)
            sdf.line_to(s * 0.2000, s * 0.8000)
            sdf.line_to(s * 0.1919, s * 0.7913)
            sdf.line_to(s * 0.1843, s * 0.7821)
            sdf.line_to(s * 0.1773, s * 0.7723)
            sdf.line_to(s * 0.1707, s * 0.7619)
            sdf.line_to(s * 0.1647, s * 0.7509)
            sdf.line_to(s * 0.1592, s * 0.7392)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2608, s * 0.1592)
            sdf.line_to(s * 0.2492, s * 0.1647)
            sdf.line_to(s * 0.2382, s * 0.1708)
            sdf.line_to(s * 0.2277, s * 0.1773)
            sdf.line_to(s * 0.2179, s * 0.1843)
            sdf.line_to(s * 0.2087, s * 0.1919)
            sdf.line_to(s * 0.2000, s * 0.2000)
            sdf.line_to(s * 0.1919, s * 0.2087)
            sdf.line_to(s * 0.1843, s * 0.2179)
            sdf.line_to(s * 0.1773, s * 0.2277)
            sdf.line_to(s * 0.1708, s * 0.2382)
            sdf.line_to(s * 0.1647, s * 0.2492)
            sdf.line_to(s * 0.1592, s * 0.2608)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/sun.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/sun.svg via scripts/gen-icon.py.
    mod.draw.IconSun = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.6667, s * 0.5000)
            sdf.arc_to(s * 0.5000, s * 0.5000, s * 0.1667, 0.0000, 3.1416)
            sdf.arc_to(s * 0.5000, s * 0.5000, s * 0.1667, 3.1416, 6.2832)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.5000, s * 0.0833)
            sdf.line_to(s * 0.5000, s * 0.1667)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.5000, s * 0.8333)
            sdf.line_to(s * 0.5000, s * 0.9167)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2054, s * 0.2054)
            sdf.line_to(s * 0.2642, s * 0.2642)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.7358, s * 0.7358)
            sdf.line_to(s * 0.7946, s * 0.7946)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.0833, s * 0.5000)
            sdf.line_to(s * 0.1667, s * 0.5000)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.8333, s * 0.5000)
            sdf.line_to(s * 0.9167, s * 0.5000)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2642, s * 0.7358)
            sdf.line_to(s * 0.2054, s * 0.7946)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.7946, s * 0.2054)
            sdf.line_to(s * 0.7358, s * 0.2642)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/tab.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/tab.svg via scripts/gen-icon.py.
    mod.draw.IconTab = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.1667, s * 0.8333)
            sdf.line_to(s * 0.1667, s * 0.2500)
            sdf.arc_to(s * 0.2500, s * 0.2500, s * 0.0833, 3.1416, 4.7124)
            sdf.line_to(s * 0.7500, s * 0.1667)
            sdf.arc_to(s * 0.7500, s * 0.2500, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.8333, s * 0.8333)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.9167, s * 0.8333)
            sdf.line_to(s * 0.0833, s * 0.8333)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/tab-text.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/tab-text.svg via scripts/gen-icon.py.
    mod.draw.IconTabText = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.3333, s * 0.3333)
            sdf.line_to(s * 0.5833, s * 0.3333)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.3333, s * 0.5000)
            sdf.line_to(s * 0.6667, s * 0.5000)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.3333, s * 0.6667)
            sdf.line_to(s * 0.5833, s * 0.6667)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.1667, s * 0.8333)
            sdf.line_to(s * 0.1667, s * 0.2500)
            sdf.arc_to(s * 0.2500, s * 0.2500, s * 0.0833, 3.1416, 4.7124)
            sdf.line_to(s * 0.7500, s * 0.1667)
            sdf.arc_to(s * 0.7500, s * 0.2500, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.8333, s * 0.8333)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.9167, s * 0.8333)
            sdf.line_to(s * 0.0833, s * 0.8333)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/tab-x.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/tab-x.svg via scripts/gen-icon.py.
    mod.draw.IconTabX = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.6042, s * 0.3958)
            sdf.line_to(s * 0.3958, s * 0.6042)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6042, s * 0.6042)
            sdf.line_to(s * 0.3958, s * 0.3958)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.1667, s * 0.8333)
            sdf.line_to(s * 0.1667, s * 0.2500)
            sdf.arc_to(s * 0.2500, s * 0.2500, s * 0.0833, 3.1416, 4.7124)
            sdf.line_to(s * 0.7500, s * 0.1667)
            sdf.arc_to(s * 0.7500, s * 0.2500, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.8333, s * 0.8333)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.9167, s * 0.8333)
            sdf.line_to(s * 0.0833, s * 0.8333)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/tag-plus.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/tag-plus.svg via scripts/gen-icon.py.
    mod.draw.IconTagPlus = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.6667, s * 0.5417)
            sdf.line_to(s * 0.9167, s * 0.5417)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6875, s * 0.2708)
            sdf.line_to(s * 0.5244, s * 0.1078)
            sdf.arc_to(s * 0.4655, s * 0.1667, s * 0.0833, -0.7852, -1.5706)
            sdf.line_to(s * 0.1667, s * 0.0833)
            sdf.arc_to(s * 0.1667, s * 0.1667, s * 0.0833, -1.5708, -3.1416)
            sdf.line_to(s * 0.0833, s * 0.4655)
            sdf.arc_to(s * 0.1667, s * 0.4655, s * 0.0833, 3.1414, 2.3560)
            sdf.line_to(s * 0.4704, s * 0.8871)
            sdf.arc_to(s * 0.5417, s * 0.8154, s * 0.1011, 2.3530, 0.7886)
            sdf.line_to(s * 0.6875, s * 0.8125)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.7917, s * 0.4167)
            sdf.line_to(s * 0.7917, s * 0.6667)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.3333, s * 0.3125)
            sdf.arc_to(s * 0.3125, s * 0.3125, s * 0.0208, 0.0000, 3.1416)
            sdf.arc_to(s * 0.3125, s * 0.3125, s * 0.0208, 3.1416, 6.2832)
            sdf.close_path()
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Faithful port of resources/icons/vector-square.svg via scripts/gen-icon.py.
    // Faithful port of resources/icons/vector-square.svg via scripts/gen-icon.py.
    mod.draw.IconVectorSquare = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.8125, s * 0.2917)
            sdf.arc_to(s * -0.1656, s * 0.5000, s * 1.0000, -0.2099, 0.2099)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.1875, s * 0.2917)
            sdf.arc_to(s * 1.1656, s * 0.5000, s * 1.0000, -2.9317, -3.3515)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2917, s * 0.8125)
            sdf.arc_to(s * 0.5000, s * -0.1656, s * 1.0000, 1.7807, 1.3609)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2917, s * 0.1875)
            sdf.arc_to(s * 0.5000, s * 1.1656, s * 1.0000, -1.7807, -1.3609)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.7500, s * 0.7083)
            sdf.line_to(s * 0.8750, s * 0.7083)
            sdf.arc_to(s * 0.8750, s * 0.7500, s * 0.0417, -1.5708, 0.0000)
            sdf.line_to(s * 0.9167, s * 0.8750)
            sdf.arc_to(s * 0.8750, s * 0.8750, s * 0.0417, 0.0000, 1.5708)
            sdf.line_to(s * 0.7500, s * 0.9167)
            sdf.arc_to(s * 0.7500, s * 0.8750, s * 0.0417, 1.5708, 3.1416)
            sdf.line_to(s * 0.7083, s * 0.7500)
            sdf.arc_to(s * 0.7500, s * 0.7500, s * 0.0417, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.7500, s * 0.0833)
            sdf.line_to(s * 0.8750, s * 0.0833)
            sdf.arc_to(s * 0.8750, s * 0.1250, s * 0.0417, -1.5708, 0.0000)
            sdf.line_to(s * 0.9167, s * 0.2500)
            sdf.arc_to(s * 0.8750, s * 0.2500, s * 0.0417, 0.0000, 1.5708)
            sdf.line_to(s * 0.7500, s * 0.2917)
            sdf.arc_to(s * 0.7500, s * 0.2500, s * 0.0417, 1.5708, 3.1416)
            sdf.line_to(s * 0.7083, s * 0.1250)
            sdf.arc_to(s * 0.7500, s * 0.1250, s * 0.0417, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.1250, s * 0.7083)
            sdf.line_to(s * 0.2500, s * 0.7083)
            sdf.arc_to(s * 0.2500, s * 0.7500, s * 0.0417, -1.5708, 0.0000)
            sdf.line_to(s * 0.2917, s * 0.8750)
            sdf.arc_to(s * 0.2500, s * 0.8750, s * 0.0417, 0.0000, 1.5708)
            sdf.line_to(s * 0.1250, s * 0.9167)
            sdf.arc_to(s * 0.1250, s * 0.8750, s * 0.0417, 1.5708, 3.1416)
            sdf.line_to(s * 0.0833, s * 0.7500)
            sdf.arc_to(s * 0.1250, s * 0.7500, s * 0.0417, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.1250, s * 0.0833)
            sdf.line_to(s * 0.2500, s * 0.0833)
            sdf.arc_to(s * 0.2500, s * 0.1250, s * 0.0417, -1.5708, 0.0000)
            sdf.line_to(s * 0.2917, s * 0.2500)
            sdf.arc_to(s * 0.2500, s * 0.2500, s * 0.0417, 0.0000, 1.5708)
            sdf.line_to(s * 0.1250, s * 0.2917)
            sdf.arc_to(s * 0.1250, s * 0.2500, s * 0.0417, 1.5708, 3.1416)
            sdf.line_to(s * 0.0833, s * 0.1250)
            sdf.arc_to(s * 0.1250, s * 0.1250, s * 0.0417, 3.1416, 4.7124)
            sdf.close_path()
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
        menu: mod.draw.IconMenu{ color: atlas.accent }
        moon: mod.draw.IconMoon{ color: atlas.accent }
        align_center_horizontal: mod.draw.IconAlignCenterHorizontal{ color: atlas.accent }
        align_center_vertical: mod.draw.IconAlignCenterVertical{ color: atlas.accent }
        align_end_horizontal: mod.draw.IconAlignEndHorizontal{ color: atlas.accent }
        align_horizontal_distribute_center: mod.draw.IconAlignHorizontalDistributeCenter{ color: atlas.accent }
        align_horizontal_distribute_end: mod.draw.IconAlignHorizontalDistributeEnd{ color: atlas.accent }
        align_horizontal_distribute_start: mod.draw.IconAlignHorizontalDistributeStart{ color: atlas.accent }
        align_horizontal_justify_center: mod.draw.IconAlignHorizontalJustifyCenter{ color: atlas.accent }
        align_horizontal_justify_end: mod.draw.IconAlignHorizontalJustifyEnd{ color: atlas.accent }
        align_horizontal_justify_start: mod.draw.IconAlignHorizontalJustifyStart{ color: atlas.accent }
        align_horizontal_space_around: mod.draw.IconAlignHorizontalSpaceAround{ color: atlas.accent }
        align_horizontal_space_between: mod.draw.IconAlignHorizontalSpaceBetween{ color: atlas.accent }
        align_start_horizontal: mod.draw.IconAlignStartHorizontal{ color: atlas.accent }
        align_start_vertical: mod.draw.IconAlignStartVertical{ color: atlas.accent }
        align_vertical_distribute_center: mod.draw.IconAlignVerticalDistributeCenter{ color: atlas.accent }
        align_vertical_distribute_end: mod.draw.IconAlignVerticalDistributeEnd{ color: atlas.accent }
        align_vertical_distribute_start: mod.draw.IconAlignVerticalDistributeStart{ color: atlas.accent }
        align_vertical_justify_center: mod.draw.IconAlignVerticalJustifyCenter{ color: atlas.accent }
        align_vertical_justify_end: mod.draw.IconAlignVerticalJustifyEnd{ color: atlas.accent }
        align_vertical_justify_start: mod.draw.IconAlignVerticalJustifyStart{ color: atlas.accent }
        align_vertical_space_around: mod.draw.IconAlignVerticalSpaceAround{ color: atlas.accent }
        align_vertical_space_between: mod.draw.IconAlignVerticalSpaceBetween{ color: atlas.accent }
        arrow_down_a_z: mod.draw.IconArrowDownAZ{ color: atlas.accent }
        arrow_down_z_a: mod.draw.IconArrowDownZA{ color: atlas.accent }
        arrow_up_a_z: mod.draw.IconArrowUpAZ{ color: atlas.accent }
        between_horizontal_end: mod.draw.IconBetweenHorizontalEnd{ color: atlas.accent }
        between_horizontal_start: mod.draw.IconBetweenHorizontalStart{ color: atlas.accent }
        between_vertical_end: mod.draw.IconBetweenVerticalEnd{ color: atlas.accent }
        between_vertical_start: mod.draw.IconBetweenVerticalStart{ color: atlas.accent }
        cable: mod.draw.IconCable{ color: atlas.accent }
        chevrons_down_up: mod.draw.IconChevronsDownUp{ color: atlas.accent }
        chevrons_left_right_ellipsis: mod.draw.IconChevronsLeftRightEllipsis{ color: atlas.accent }
        chevrons_up: mod.draw.IconChevronsUp{ color: atlas.accent }
        chevrons_up_down: mod.draw.IconChevronsUpDown{ color: atlas.accent }
        circle_x: mod.draw.IconCircleX{ color: atlas.accent }
        eye: mod.draw.IconEye{ color: atlas.accent }
        eye_off: mod.draw.IconEyeOff{ color: atlas.accent }
        frame: mod.draw.IconFrame{ color: atlas.accent }
        funnel: mod.draw.IconFunnel{ color: atlas.accent }
        funnel_x: mod.draw.IconFunnelX{ color: atlas.accent }
        grip_vertical: mod.draw.IconGripVertical{ color: atlas.accent }
        info: mod.draw.IconInfo{ color: atlas.accent }
        mouse_pointer_2: mod.draw.IconMousePointer2{ color: atlas.accent }
        package_check: mod.draw.IconPackageCheck{ color: atlas.accent }
        package_open: mod.draw.IconPackageOpen{ color: atlas.accent }
        panel_top: mod.draw.IconPanelTop{ color: atlas.accent }
        radar: mod.draw.IconRadar{ color: atlas.accent }
        save: mod.draw.IconSave{ color: atlas.accent }
        save_check: mod.draw.IconSaveCheck{ color: atlas.accent }
        save_pen: mod.draw.IconSavePen{ color: atlas.accent }
        scan: mod.draw.IconScan{ color: atlas.accent }
        sliders_horizontal: mod.draw.IconSlidersHorizontal{ color: atlas.accent }
        square: mod.draw.IconSquare{ color: atlas.accent }
        square_dashed_top_solid: mod.draw.IconSquareDashedTopSolid{ color: atlas.accent }
        square_menu: mod.draw.IconSquareMenu{ color: atlas.accent }
        squircle: mod.draw.IconSquircle{ color: atlas.accent }
        squircle_dashed: mod.draw.IconSquircleDashed{ color: atlas.accent }
        sun: mod.draw.IconSun{ color: atlas.accent }
        tab: mod.draw.IconTab{ color: atlas.accent }
        tab_text: mod.draw.IconTabText{ color: atlas.accent }
        tab_x: mod.draw.IconTabX{ color: atlas.accent }
        tag_plus: mod.draw.IconTagPlus{ color: atlas.accent }
        vector_square: mod.draw.IconVectorSquare{ color: atlas.accent }
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
    #[live]
    pub menu: DrawColor,
    #[live]
    pub moon: DrawColor,
    #[live]
    pub align_center_horizontal: DrawColor,
    #[live]
    pub align_center_vertical: DrawColor,
    #[live]
    pub align_end_horizontal: DrawColor,
    #[live]
    pub align_horizontal_distribute_center: DrawColor,
    #[live]
    pub align_horizontal_distribute_end: DrawColor,
    #[live]
    pub align_horizontal_distribute_start: DrawColor,
    #[live]
    pub align_horizontal_justify_center: DrawColor,
    #[live]
    pub align_horizontal_justify_end: DrawColor,
    #[live]
    pub align_horizontal_justify_start: DrawColor,
    #[live]
    pub align_horizontal_space_around: DrawColor,
    #[live]
    pub align_horizontal_space_between: DrawColor,
    #[live]
    pub align_start_horizontal: DrawColor,
    #[live]
    pub align_start_vertical: DrawColor,
    #[live]
    pub align_vertical_distribute_center: DrawColor,
    #[live]
    pub align_vertical_distribute_end: DrawColor,
    #[live]
    pub align_vertical_distribute_start: DrawColor,
    #[live]
    pub align_vertical_justify_center: DrawColor,
    #[live]
    pub align_vertical_justify_end: DrawColor,
    #[live]
    pub align_vertical_justify_start: DrawColor,
    #[live]
    pub align_vertical_space_around: DrawColor,
    #[live]
    pub align_vertical_space_between: DrawColor,
    #[live]
    pub arrow_down_a_z: DrawColor,
    #[live]
    pub arrow_down_z_a: DrawColor,
    #[live]
    pub arrow_up_a_z: DrawColor,
    #[live]
    pub between_horizontal_end: DrawColor,
    #[live]
    pub between_horizontal_start: DrawColor,
    #[live]
    pub between_vertical_end: DrawColor,
    #[live]
    pub between_vertical_start: DrawColor,
    #[live]
    pub cable: DrawColor,
    #[live]
    pub chevrons_down_up: DrawColor,
    #[live]
    pub chevrons_left_right_ellipsis: DrawColor,
    #[live]
    pub chevrons_up: DrawColor,
    #[live]
    pub chevrons_up_down: DrawColor,
    #[live]
    pub circle_x: DrawColor,
    #[live]
    pub eye: DrawColor,
    #[live]
    pub eye_off: DrawColor,
    #[live]
    pub frame: DrawColor,
    #[live]
    pub funnel: DrawColor,
    #[live]
    pub funnel_x: DrawColor,
    #[live]
    pub grip_vertical: DrawColor,
    #[live]
    pub info: DrawColor,
    #[live]
    pub mouse_pointer_2: DrawColor,
    #[live]
    pub package_check: DrawColor,
    #[live]
    pub package_open: DrawColor,
    #[live]
    pub panel_top: DrawColor,
    #[live]
    pub radar: DrawColor,
    #[live]
    pub save: DrawColor,
    #[live]
    pub save_check: DrawColor,
    #[live]
    pub save_pen: DrawColor,
    #[live]
    pub scan: DrawColor,
    #[live]
    pub sliders_horizontal: DrawColor,
    #[live]
    pub square: DrawColor,
    #[live]
    pub square_dashed_top_solid: DrawColor,
    #[live]
    pub square_menu: DrawColor,
    #[live]
    pub squircle: DrawColor,
    #[live]
    pub squircle_dashed: DrawColor,
    #[live]
    pub sun: DrawColor,
    #[live]
    pub tab: DrawColor,
    #[live]
    pub tab_text: DrawColor,
    #[live]
    pub tab_x: DrawColor,
    #[live]
    pub tag_plus: DrawColor,
    #[live]
    pub vector_square: DrawColor,
}

impl TreeIcons {
    /// All nine glyphs paired with a short label, in a stable order. Used by the
    /// `icon_harness` bin's proof-grid; the shipping tree/doc-tabs pick glyphs by
    /// `TreeKind` via `icon_for` in `tree_panel.rs` instead.
    #[allow(dead_code)]
    pub fn labeled_mut(&mut self) -> [(&'static str, &mut DrawColor); 87] {
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
            ("message-square-text", &mut self.message),
            ("package-plus", &mut self.package_plus),
            ("paintbrush-vertical", &mut self.paintbrush),
            ("pin", &mut self.pin),
            ("pin-off", &mut self.pin_off),
            ("share", &mut self.share),
            ("spline", &mut self.spline),
            ("spline-pointer", &mut self.spline_pointer),
            ("square-minus", &mut self.square_minus),
            ("square-plus", &mut self.square_plus),
            ("trash", &mut self.trash),
            ("list-chevrons-down-up", &mut self.list_collapse),
            ("list-chevrons-up-down", &mut self.list_expand),
            ("pencil", &mut self.pencil),
            ("menu", &mut self.menu),
            ("moon", &mut self.moon),
            ("align-center-horizontal", &mut self.align_center_horizontal),
            ("align-center-vertical", &mut self.align_center_vertical),
            ("align-end-horizontal", &mut self.align_end_horizontal),
            ("align-horizontal-distribute-center", &mut self.align_horizontal_distribute_center),
            ("align-horizontal-distribute-end", &mut self.align_horizontal_distribute_end),
            ("align-horizontal-distribute-start", &mut self.align_horizontal_distribute_start),
            ("align-horizontal-justify-center", &mut self.align_horizontal_justify_center),
            ("align-horizontal-justify-end", &mut self.align_horizontal_justify_end),
            ("align-horizontal-justify-start", &mut self.align_horizontal_justify_start),
            ("align-horizontal-space-around", &mut self.align_horizontal_space_around),
            ("align-horizontal-space-between", &mut self.align_horizontal_space_between),
            ("align-start-horizontal", &mut self.align_start_horizontal),
            ("align-start-vertical", &mut self.align_start_vertical),
            ("align-vertical-distribute-center", &mut self.align_vertical_distribute_center),
            ("align-vertical-distribute-end", &mut self.align_vertical_distribute_end),
            ("align-vertical-distribute-start", &mut self.align_vertical_distribute_start),
            ("align-vertical-justify-center", &mut self.align_vertical_justify_center),
            ("align-vertical-justify-end", &mut self.align_vertical_justify_end),
            ("align-vertical-justify-start", &mut self.align_vertical_justify_start),
            ("align-vertical-space-around", &mut self.align_vertical_space_around),
            ("align-vertical-space-between", &mut self.align_vertical_space_between),
            ("arrow-down-a-z", &mut self.arrow_down_a_z),
            ("arrow-down-z-a", &mut self.arrow_down_z_a),
            ("arrow-up-a-z", &mut self.arrow_up_a_z),
            ("between-horizontal-end", &mut self.between_horizontal_end),
            ("between-horizontal-start", &mut self.between_horizontal_start),
            ("between-vertical-end", &mut self.between_vertical_end),
            ("between-vertical-start", &mut self.between_vertical_start),
            ("cable", &mut self.cable),
            ("chevrons-down-up", &mut self.chevrons_down_up),
            ("chevrons-left-right-ellipsis", &mut self.chevrons_left_right_ellipsis),
            ("chevrons-up", &mut self.chevrons_up),
            ("chevrons-up-down", &mut self.chevrons_up_down),
            ("circle-x", &mut self.circle_x),
            ("eye", &mut self.eye),
            ("eye-off", &mut self.eye_off),
            ("frame", &mut self.frame),
            ("funnel", &mut self.funnel),
            ("funnel-x", &mut self.funnel_x),
            ("grip-vertical", &mut self.grip_vertical),
            ("info", &mut self.info),
            ("mouse-pointer-2", &mut self.mouse_pointer_2),
            ("package-check", &mut self.package_check),
            ("package-open", &mut self.package_open),
            ("panel-top", &mut self.panel_top),
            ("radar", &mut self.radar),
            ("save", &mut self.save),
            ("save-check", &mut self.save_check),
            ("save-pen", &mut self.save_pen),
            ("scan", &mut self.scan),
            ("sliders-horizontal", &mut self.sliders_horizontal),
            ("square", &mut self.square),
            ("square-dashed-top-solid", &mut self.square_dashed_top_solid),
            ("square-menu", &mut self.square_menu),
            ("squircle", &mut self.squircle),
            ("squircle-dashed", &mut self.squircle_dashed),
            ("sun", &mut self.sun),
            ("tab", &mut self.tab),
            ("tab-text", &mut self.tab_text),
            ("tab-x", &mut self.tab_x),
            ("tag-plus", &mut self.tag_plus),
            ("vector-square", &mut self.vector_square),
        ]
    }
}
