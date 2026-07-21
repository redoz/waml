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

    // Package: the Lucide box/cube -- outline, center seam, top-V seam, and the
    // corner flap edge.
    // Faithful port of resources/icons/package.svg via scripts/gen-icon.py.
    mod.draw.IconPackage = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.4583, s * 0.9054)
            sdf.arc_to(s * 0.5000, s * 0.8332, s * 0.0833, 2.0944, 1.0472)
            sdf.line_to(s * 0.8333, s * 0.7388)
            sdf.arc_to(s * 0.7917, s * 0.6666, s * 0.0833, 1.0472, 0.0010)
            sdf.line_to(s * 0.8750, s * 0.3333)
            sdf.arc_to(s * 0.7917, s * 0.3334, s * 0.0833, -0.0010, -1.0472)
            sdf.line_to(s * 0.5417, s * 0.0946)
            sdf.arc_to(s * 0.5000, s * 0.1668, s * 0.0833, -1.0472, -2.0944)
            sdf.line_to(s * 0.1667, s * 0.2613)
            sdf.arc_to(s * 0.2083, s * 0.3334, s * 0.0833, -2.0944, -3.1406)
            sdf.line_to(s * 0.1250, s * 0.6667)
            sdf.arc_to(s * 0.2083, s * 0.6666, s * 0.0833, 3.1406, 2.0944)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.5000, s * 0.9167)
            sdf.line_to(s * 0.5000, s * 0.5000)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.1371, s * 0.2917)
            sdf.line_to(s * 0.5000, s * 0.5000)
            sdf.line_to(s * 0.8629, s * 0.2917)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.3125, s * 0.1779)
            sdf.line_to(s * 0.6875, s * 0.3925)
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

    // Door closed: a door slab with a knob dot, over a floor line.
    // Faithful port of resources/icons/door-closed.svg via scripts/gen-icon.py.
    mod.draw.IconDoorClosed = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.4167, s * 0.5000)
            sdf.line_to(s * 0.4171, s * 0.5000)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.7500, s * 0.8333)
            sdf.line_to(s * 0.7500, s * 0.2500)
            sdf.arc_to(s * 0.6667, s * 0.2500, s * 0.0833, 0.0000, -1.5708)
            sdf.line_to(s * 0.3333, s * 0.1667)
            sdf.arc_to(s * 0.3333, s * 0.2500, s * 0.0833, -1.5708, -3.1416)
            sdf.line_to(s * 0.2500, s * 0.8333)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.0833, s * 0.8333)
            sdf.line_to(s * 0.9167, s * 0.8333)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Sticky note: a page with a folded bottom-right corner.
    // Faithful port of resources/icons/sticky-note.svg via scripts/gen-icon.py.
    mod.draw.IconStickyNote = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.8750, s * 0.3750)
            sdf.arc_to(s * 0.7750, s * 0.3748, s * 0.1000, 0.0025, -0.7872)
            sdf.line_to(s * 0.6961, s * 0.1544)
            sdf.arc_to(s * 0.6252, s * 0.2250, s * 0.1000, -0.7836, -1.5732)
            sdf.line_to(s * 0.2083, s * 0.1250)
            sdf.arc_to(s * 0.2083, s * 0.2083, s * 0.0833, -1.5708, -3.1416)
            sdf.line_to(s * 0.1250, s * 0.7917)
            sdf.arc_to(s * 0.2083, s * 0.7917, s * 0.0833, 3.1416, 1.5708)
            sdf.line_to(s * 0.7917, s * 0.8750)
            sdf.arc_to(s * 0.7917, s * 0.7917, s * 0.0833, 1.5708, 0.0000)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6250, s * 0.1250)
            sdf.line_to(s * 0.6250, s * 0.3333)
            sdf.arc_to(s * 0.6667, s * 0.3333, s * 0.0417, 3.1416, 1.5708)
            sdf.line_to(s * 0.8750, s * 0.3750)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // List: three bullet dots + three rows.
    // Faithful port of resources/icons/list.svg via scripts/gen-icon.py.
    mod.draw.IconList = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.1250, s * 0.2083)
            sdf.line_to(s * 0.1254, s * 0.2083)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.1250, s * 0.5000)
            sdf.line_to(s * 0.1254, s * 0.5000)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.1250, s * 0.7917)
            sdf.line_to(s * 0.1254, s * 0.7917)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.3333, s * 0.2083)
            sdf.line_to(s * 0.8750, s * 0.2083)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.3333, s * 0.5000)
            sdf.line_to(s * 0.8750, s * 0.5000)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.3333, s * 0.7917)
            sdf.line_to(s * 0.8750, s * 0.7917)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Braces: a pair of curly braces -- value/primitive type.
    // Faithful port of resources/icons/braces.svg via scripts/gen-icon.py.
    mod.draw.IconBraces = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.3333, s * 0.1250)
            sdf.line_to(s * 0.2917, s * 0.1250)
            sdf.arc_to(s * 0.2917, s * 0.2083, s * 0.0833, -1.5708, -3.1416)
            sdf.line_to(s * 0.2083, s * 0.4167)
            sdf.arc_to(s * 0.1250, s * 0.4167, s * 0.0833, 0.0000, 1.5708)
            sdf.arc_to(s * 0.1250, s * 0.5833, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.2083, s * 0.7917)
            sdf.line_to(s * 0.2091, s * 0.8029)
            sdf.line_to(s * 0.2113, s * 0.8138)
            sdf.line_to(s * 0.2149, s * 0.8240)
            sdf.line_to(s * 0.2198, s * 0.8336)
            sdf.line_to(s * 0.2258, s * 0.8425)
            sdf.line_to(s * 0.2328, s * 0.8505)
            sdf.line_to(s * 0.2408, s * 0.8576)
            sdf.line_to(s * 0.2497, s * 0.8636)
            sdf.line_to(s * 0.2593, s * 0.8684)
            sdf.line_to(s * 0.2696, s * 0.8720)
            sdf.line_to(s * 0.2804, s * 0.8742)
            sdf.line_to(s * 0.2917, s * 0.8750)
            sdf.line_to(s * 0.3333, s * 0.8750)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6667, s * 0.8750)
            sdf.line_to(s * 0.7083, s * 0.8750)
            sdf.arc_to(s * 0.7083, s * 0.7917, s * 0.0833, 1.5708, 0.0000)
            sdf.line_to(s * 0.7917, s * 0.5833)
            sdf.line_to(s * 0.7924, s * 0.5721)
            sdf.line_to(s * 0.7947, s * 0.5612)
            sdf.line_to(s * 0.7982, s * 0.5510)
            sdf.line_to(s * 0.8031, s * 0.5414)
            sdf.line_to(s * 0.8091, s * 0.5325)
            sdf.line_to(s * 0.8161, s * 0.5245)
            sdf.line_to(s * 0.8242, s * 0.5174)
            sdf.line_to(s * 0.8330, s * 0.5114)
            sdf.line_to(s * 0.8426, s * 0.5066)
            sdf.line_to(s * 0.8529, s * 0.5030)
            sdf.line_to(s * 0.8637, s * 0.5008)
            sdf.line_to(s * 0.8750, s * 0.5000)
            sdf.arc_to(s * 0.8750, s * 0.4167, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.7917, s * 0.2083)
            sdf.arc_to(s * 0.7083, s * 0.2083, s * 0.0833, 0.0000, -1.5708)
            sdf.line_to(s * 0.6667, s * 0.1250)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Workflow: two rounded-square nodes joined by an elbow link -- graph/canvas.
    // Faithful port of resources/icons/workflow.svg via scripts/gen-icon.py.
    mod.draw.IconWorkflow = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.2083, s * 0.1250)
            sdf.line_to(s * 0.3750, s * 0.1250)
            sdf.arc_to(s * 0.3750, s * 0.2083, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.4583, s * 0.3750)
            sdf.arc_to(s * 0.3750, s * 0.3750, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.2083, s * 0.4583)
            sdf.arc_to(s * 0.2083, s * 0.3750, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.1250, s * 0.2083)
            sdf.arc_to(s * 0.2083, s * 0.2083, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2917, s * 0.4583)
            sdf.line_to(s * 0.2917, s * 0.6250)
            sdf.arc_to(s * 0.3750, s * 0.6250, s * 0.0833, 3.1416, 1.5708)
            sdf.line_to(s * 0.5417, s * 0.7083)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6250, s * 0.5417)
            sdf.line_to(s * 0.7917, s * 0.5417)
            sdf.arc_to(s * 0.7917, s * 0.6250, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.8750, s * 0.7917)
            sdf.arc_to(s * 0.7917, s * 0.7917, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.6250, s * 0.8750)
            sdf.arc_to(s * 0.6250, s * 0.7917, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.5417, s * 0.6250)
            sdf.arc_to(s * 0.6250, s * 0.6250, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Activity: a heartbeat/pulse polyline -- behavior/activity.
    // Faithful port of resources/icons/activity.svg via scripts/gen-icon.py.
    mod.draw.IconActivity = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.9167, s * 0.5000)
            sdf.line_to(s * 0.8133, s * 0.5000)
            sdf.arc_to(s * 0.8132, s * 0.5833, s * 0.0833, -1.5687, -2.8682)
            sdf.line_to(s * 0.6350, s * 0.9092)
            sdf.arc_to(s * 0.6250, s * 0.9063, s * 0.0104, 0.2838, 2.8578)
            sdf.line_to(s * 0.3850, s * 0.0908)
            sdf.arc_to(s * 0.3750, s * 0.0938, s * 0.0104, -0.2838, -2.8578)
            sdf.line_to(s * 0.2671, s * 0.4392)
            sdf.arc_to(s * 0.1868, s * 0.4167, s * 0.0833, 0.2734, 1.5679)
            sdf.line_to(s * 0.0833, s * 0.5000)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Arrow left-right: two opposing arrows -- message exchange over time.
    // Faithful port of resources/icons/arrow-left-right.svg via scripts/gen-icon.py.
    mod.draw.IconArrowLeftRight = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.3333, s * 0.1250)
            sdf.line_to(s * 0.1667, s * 0.2917)
            sdf.line_to(s * 0.3333, s * 0.4583)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.1667, s * 0.2917)
            sdf.line_to(s * 0.8333, s * 0.2917)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6667, s * 0.8750)
            sdf.line_to(s * 0.8333, s * 0.7083)
            sdf.line_to(s * 0.6667, s * 0.5417)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.8333, s * 0.7083)
            sdf.line_to(s * 0.1667, s * 0.7083)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Folder: the Lucide folder -- single body outline with the raised tab.
    // Faithful port of resources/icons/folder.svg via scripts/gen-icon.py.
    mod.draw.IconFolder = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.8333, s * 0.8333)
            sdf.arc_to(s * 0.8333, s * 0.7500, s * 0.0833, 1.5708, 0.0000)
            sdf.line_to(s * 0.9167, s * 0.3333)
            sdf.arc_to(s * 0.8333, s * 0.3333, s * 0.0833, 0.0000, -1.5708)
            sdf.line_to(s * 0.5042, s * 0.2500)
            sdf.arc_to(s * 0.5034, s * 0.1667, s * 0.0833, 1.5610, 2.5593)
            sdf.line_to(s * 0.4000, s * 0.1625)
            sdf.arc_to(s * 0.3304, s * 0.2083, s * 0.0833, -0.5824, -1.5706)
            sdf.line_to(s * 0.1667, s * 0.1250)
            sdf.arc_to(s * 0.1667, s * 0.2083, s * 0.0833, -1.5708, -3.1416)
            sdf.line_to(s * 0.0833, s * 0.7500)
            sdf.arc_to(s * 0.1667, s * 0.7500, s * 0.0833, 3.1416, 1.5708)
            sdf.close_path()
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Folder (closed): the Lucide folder-closed -- the folder body plus the
    // horizontal seam across the top. Kept in the catalog for future use even
    // though packages currently map to the open `folder`. Faithful port of
    // resources/icons/folder-closed.svg via scripts/gen-icon.py.
    mod.draw.IconFolderClosed = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.8333, s * 0.8333)
            sdf.arc_to(s * 0.8333, s * 0.7500, s * 0.0833, 1.5708, 0.0000)
            sdf.line_to(s * 0.9167, s * 0.3333)
            sdf.arc_to(s * 0.8333, s * 0.3333, s * 0.0833, 0.0000, -1.5708)
            sdf.line_to(s * 0.5042, s * 0.2500)
            sdf.arc_to(s * 0.5034, s * 0.1667, s * 0.0833, 1.5610, 2.5593)
            sdf.line_to(s * 0.4000, s * 0.1625)
            sdf.arc_to(s * 0.3304, s * 0.2083, s * 0.0833, -0.5824, -1.5706)
            sdf.line_to(s * 0.1667, s * 0.1250)
            sdf.arc_to(s * 0.1667, s * 0.2083, s * 0.0833, -1.5708, -3.1416)
            sdf.line_to(s * 0.0833, s * 0.7500)
            sdf.arc_to(s * 0.1667, s * 0.7500, s * 0.0833, 3.1416, 1.5708)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.0833, s * 0.4167)
            sdf.line_to(s * 0.9167, s * 0.4167)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    mod.widgets.IconSetBase = #(IconSet::script_component(vm))

    // Each field is a `DrawColor` pointing at its icon shader; the accent tint
    // is set once here and stays accent regardless of row state.
    mod.widgets.IconSet = set_type_default() do mod.widgets.IconSetBase{
        package: mod.draw.IconPackage{ color: atlas.accent }
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
        door_closed: mod.draw.IconDoorClosed{ color: atlas.accent }
        sticky_note: mod.draw.IconStickyNote{ color: atlas.accent }
        list: mod.draw.IconList{ color: atlas.accent }
        braces: mod.draw.IconBraces{ color: atlas.accent }
        workflow: mod.draw.IconWorkflow{ color: atlas.accent }
        activity: mod.draw.IconActivity{ color: atlas.accent }
        arrow_left_right: mod.draw.IconArrowLeftRight{ color: atlas.accent }
        folder: mod.draw.IconFolder{ color: atlas.accent }
        folder_closed: mod.draw.IconFolderClosed{ color: atlas.accent }
    }
}

/// The per-kind glyph set, drawn in immediate mode via `DrawColor::draw_abs`.
/// Field order matches the `IconSet` DSL above.
#[derive(Script, ScriptHook)]
pub struct IconSet {
    #[live]
    pub package: DrawColor,
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
    #[live]
    pub door_closed: DrawColor,
    #[live]
    pub sticky_note: DrawColor,
    #[live]
    pub list: DrawColor,
    #[live]
    pub braces: DrawColor,
    #[live]
    pub workflow: DrawColor,
    #[live]
    pub activity: DrawColor,
    #[live]
    pub arrow_left_right: DrawColor,
    #[live]
    pub folder: DrawColor,
    #[live]
    pub folder_closed: DrawColor,
}

// Not every bin that `#[path]`-includes this file exercises the whole catalog
// API (e.g. `logo_harness` touches only `moon`), so per-bin dead-code analysis
// flags these as unused; the main `waml-editor` bin uses them.
#[allow(dead_code)]
impl IconSet {
    /// The one place a glyph maps to its `DrawColor` shader. Field order ==
    /// `Icon::ALL` order (the load-bearing order invariant).
    pub fn get(&mut self, icon: Icon) -> &mut DrawColor {
        match icon {
            Icon::Package => &mut self.package,
            Icon::Message => &mut self.message,
            Icon::PackagePlus => &mut self.package_plus,
            Icon::Paintbrush => &mut self.paintbrush,
            Icon::Pin => &mut self.pin,
            Icon::PinOff => &mut self.pin_off,
            Icon::Share => &mut self.share,
            Icon::Spline => &mut self.spline,
            Icon::SplinePointer => &mut self.spline_pointer,
            Icon::SquareMinus => &mut self.square_minus,
            Icon::SquarePlus => &mut self.square_plus,
            Icon::Trash => &mut self.trash,
            Icon::ListCollapse => &mut self.list_collapse,
            Icon::ListExpand => &mut self.list_expand,
            Icon::Pencil => &mut self.pencil,
            Icon::Menu => &mut self.menu,
            Icon::Moon => &mut self.moon,
            Icon::AlignCenterHorizontal => &mut self.align_center_horizontal,
            Icon::AlignCenterVertical => &mut self.align_center_vertical,
            Icon::AlignEndHorizontal => &mut self.align_end_horizontal,
            Icon::AlignHorizontalDistributeCenter => &mut self.align_horizontal_distribute_center,
            Icon::AlignHorizontalDistributeEnd => &mut self.align_horizontal_distribute_end,
            Icon::AlignHorizontalDistributeStart => &mut self.align_horizontal_distribute_start,
            Icon::AlignHorizontalJustifyCenter => &mut self.align_horizontal_justify_center,
            Icon::AlignHorizontalJustifyEnd => &mut self.align_horizontal_justify_end,
            Icon::AlignHorizontalJustifyStart => &mut self.align_horizontal_justify_start,
            Icon::AlignHorizontalSpaceAround => &mut self.align_horizontal_space_around,
            Icon::AlignHorizontalSpaceBetween => &mut self.align_horizontal_space_between,
            Icon::AlignStartHorizontal => &mut self.align_start_horizontal,
            Icon::AlignStartVertical => &mut self.align_start_vertical,
            Icon::AlignVerticalDistributeCenter => &mut self.align_vertical_distribute_center,
            Icon::AlignVerticalDistributeEnd => &mut self.align_vertical_distribute_end,
            Icon::AlignVerticalDistributeStart => &mut self.align_vertical_distribute_start,
            Icon::AlignVerticalJustifyCenter => &mut self.align_vertical_justify_center,
            Icon::AlignVerticalJustifyEnd => &mut self.align_vertical_justify_end,
            Icon::AlignVerticalJustifyStart => &mut self.align_vertical_justify_start,
            Icon::AlignVerticalSpaceAround => &mut self.align_vertical_space_around,
            Icon::AlignVerticalSpaceBetween => &mut self.align_vertical_space_between,
            Icon::ArrowDownAZ => &mut self.arrow_down_a_z,
            Icon::ArrowDownZA => &mut self.arrow_down_z_a,
            Icon::ArrowUpAZ => &mut self.arrow_up_a_z,
            Icon::BetweenHorizontalEnd => &mut self.between_horizontal_end,
            Icon::BetweenHorizontalStart => &mut self.between_horizontal_start,
            Icon::BetweenVerticalEnd => &mut self.between_vertical_end,
            Icon::BetweenVerticalStart => &mut self.between_vertical_start,
            Icon::Cable => &mut self.cable,
            Icon::ChevronsDownUp => &mut self.chevrons_down_up,
            Icon::ChevronsLeftRightEllipsis => &mut self.chevrons_left_right_ellipsis,
            Icon::ChevronsUp => &mut self.chevrons_up,
            Icon::ChevronsUpDown => &mut self.chevrons_up_down,
            Icon::CircleX => &mut self.circle_x,
            Icon::Eye => &mut self.eye,
            Icon::EyeOff => &mut self.eye_off,
            Icon::Frame => &mut self.frame,
            Icon::Funnel => &mut self.funnel,
            Icon::FunnelX => &mut self.funnel_x,
            Icon::GripVertical => &mut self.grip_vertical,
            Icon::Info => &mut self.info,
            Icon::MousePointer2 => &mut self.mouse_pointer_2,
            Icon::PackageCheck => &mut self.package_check,
            Icon::PackageOpen => &mut self.package_open,
            Icon::PanelTop => &mut self.panel_top,
            Icon::Radar => &mut self.radar,
            Icon::Save => &mut self.save,
            Icon::SaveCheck => &mut self.save_check,
            Icon::SavePen => &mut self.save_pen,
            Icon::Scan => &mut self.scan,
            Icon::SlidersHorizontal => &mut self.sliders_horizontal,
            Icon::Square => &mut self.square,
            Icon::SquareDashedTopSolid => &mut self.square_dashed_top_solid,
            Icon::SquareMenu => &mut self.square_menu,
            Icon::Squircle => &mut self.squircle,
            Icon::SquircleDashed => &mut self.squircle_dashed,
            Icon::Sun => &mut self.sun,
            Icon::Tab => &mut self.tab,
            Icon::TabText => &mut self.tab_text,
            Icon::TabX => &mut self.tab_x,
            Icon::TagPlus => &mut self.tag_plus,
            Icon::VectorSquare => &mut self.vector_square,
            Icon::DoorClosed => &mut self.door_closed,
            Icon::StickyNote => &mut self.sticky_note,
            Icon::List => &mut self.list,
            Icon::Braces => &mut self.braces,
            Icon::Workflow => &mut self.workflow,
            Icon::Activity => &mut self.activity,
            Icon::ArrowLeftRight => &mut self.arrow_left_right,
            Icon::Folder => &mut self.folder,
            Icon::FolderClosed => &mut self.folder_closed,
        }
    }

    /// Set `color` on the glyph's shader, then draw it into `rect`. The single
    /// tint+draw path; callers pass a tint copied from a DSL atlas-token holder
    /// (no RGBA crosses Rust).
    pub fn draw(&mut self, cx: &mut Cx2d, icon: Icon, rect: Rect, color: Vec4) {
        let dc = self.get(icon);
        dc.color = color;
        dc.draw_abs(cx, rect);
    }
}

/// One variant per catalog glyph, in the exact `IconSet` field order (the
/// load-bearing order invariant: enum == field == DSL == `ALL` == `label`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)] // some bins include this file without touching every variant
pub enum Icon {
    Package,
    Message,
    PackagePlus,
    Paintbrush,
    Pin,
    PinOff,
    Share,
    Spline,
    SplinePointer,
    SquareMinus,
    SquarePlus,
    Trash,
    ListCollapse,
    ListExpand,
    Pencil,
    Menu,
    Moon,
    AlignCenterHorizontal,
    AlignCenterVertical,
    AlignEndHorizontal,
    AlignHorizontalDistributeCenter,
    AlignHorizontalDistributeEnd,
    AlignHorizontalDistributeStart,
    AlignHorizontalJustifyCenter,
    AlignHorizontalJustifyEnd,
    AlignHorizontalJustifyStart,
    AlignHorizontalSpaceAround,
    AlignHorizontalSpaceBetween,
    AlignStartHorizontal,
    AlignStartVertical,
    AlignVerticalDistributeCenter,
    AlignVerticalDistributeEnd,
    AlignVerticalDistributeStart,
    AlignVerticalJustifyCenter,
    AlignVerticalJustifyEnd,
    AlignVerticalJustifyStart,
    AlignVerticalSpaceAround,
    AlignVerticalSpaceBetween,
    ArrowDownAZ,
    ArrowDownZA,
    ArrowUpAZ,
    BetweenHorizontalEnd,
    BetweenHorizontalStart,
    BetweenVerticalEnd,
    BetweenVerticalStart,
    Cable,
    ChevronsDownUp,
    ChevronsLeftRightEllipsis,
    ChevronsUp,
    ChevronsUpDown,
    CircleX,
    Eye,
    EyeOff,
    Frame,
    Funnel,
    FunnelX,
    GripVertical,
    Info,
    MousePointer2,
    PackageCheck,
    PackageOpen,
    PanelTop,
    Radar,
    Save,
    SaveCheck,
    SavePen,
    Scan,
    SlidersHorizontal,
    Square,
    SquareDashedTopSolid,
    SquareMenu,
    Squircle,
    SquircleDashed,
    Sun,
    Tab,
    TabText,
    TabX,
    TagPlus,
    VectorSquare,
    DoorClosed,
    StickyNote,
    List,
    Braces,
    Workflow,
    Activity,
    ArrowLeftRight,
    Folder,
    FolderClosed,
}

#[allow(dead_code)] // ALL/label are unused in bins that don't iterate the catalog
impl Icon {
    /// Every glyph, in field order. The single source of glyph identity; the
    /// `icon_harness` proof grid iterates this.
    pub const ALL: [Icon; 88] = [
        Icon::Package,
        Icon::Message,
        Icon::PackagePlus,
        Icon::Paintbrush,
        Icon::Pin,
        Icon::PinOff,
        Icon::Share,
        Icon::Spline,
        Icon::SplinePointer,
        Icon::SquareMinus,
        Icon::SquarePlus,
        Icon::Trash,
        Icon::ListCollapse,
        Icon::ListExpand,
        Icon::Pencil,
        Icon::Menu,
        Icon::Moon,
        Icon::AlignCenterHorizontal,
        Icon::AlignCenterVertical,
        Icon::AlignEndHorizontal,
        Icon::AlignHorizontalDistributeCenter,
        Icon::AlignHorizontalDistributeEnd,
        Icon::AlignHorizontalDistributeStart,
        Icon::AlignHorizontalJustifyCenter,
        Icon::AlignHorizontalJustifyEnd,
        Icon::AlignHorizontalJustifyStart,
        Icon::AlignHorizontalSpaceAround,
        Icon::AlignHorizontalSpaceBetween,
        Icon::AlignStartHorizontal,
        Icon::AlignStartVertical,
        Icon::AlignVerticalDistributeCenter,
        Icon::AlignVerticalDistributeEnd,
        Icon::AlignVerticalDistributeStart,
        Icon::AlignVerticalJustifyCenter,
        Icon::AlignVerticalJustifyEnd,
        Icon::AlignVerticalJustifyStart,
        Icon::AlignVerticalSpaceAround,
        Icon::AlignVerticalSpaceBetween,
        Icon::ArrowDownAZ,
        Icon::ArrowDownZA,
        Icon::ArrowUpAZ,
        Icon::BetweenHorizontalEnd,
        Icon::BetweenHorizontalStart,
        Icon::BetweenVerticalEnd,
        Icon::BetweenVerticalStart,
        Icon::Cable,
        Icon::ChevronsDownUp,
        Icon::ChevronsLeftRightEllipsis,
        Icon::ChevronsUp,
        Icon::ChevronsUpDown,
        Icon::CircleX,
        Icon::Eye,
        Icon::EyeOff,
        Icon::Frame,
        Icon::Funnel,
        Icon::FunnelX,
        Icon::GripVertical,
        Icon::Info,
        Icon::MousePointer2,
        Icon::PackageCheck,
        Icon::PackageOpen,
        Icon::PanelTop,
        Icon::Radar,
        Icon::Save,
        Icon::SaveCheck,
        Icon::SavePen,
        Icon::Scan,
        Icon::SlidersHorizontal,
        Icon::Square,
        Icon::SquareDashedTopSolid,
        Icon::SquareMenu,
        Icon::Squircle,
        Icon::SquircleDashed,
        Icon::Sun,
        Icon::Tab,
        Icon::TabText,
        Icon::TabX,
        Icon::TagPlus,
        Icon::VectorSquare,
        Icon::DoorClosed,
        Icon::StickyNote,
        Icon::List,
        Icon::Braces,
        Icon::Workflow,
        Icon::Activity,
        Icon::ArrowLeftRight,
        Icon::Folder,
        Icon::FolderClosed,
    ];

    /// The `icon_harness` display slug (the Lucide source name), preserved
    /// verbatim from the old `labeled_mut` list so the proof grid is unchanged.
    pub fn label(self) -> &'static str {
        match self {
            Icon::Package => "package",
            Icon::Message => "message-square-text",
            Icon::PackagePlus => "package-plus",
            Icon::Paintbrush => "paintbrush-vertical",
            Icon::Pin => "pin",
            Icon::PinOff => "pin-off",
            Icon::Share => "share",
            Icon::Spline => "spline",
            Icon::SplinePointer => "spline-pointer",
            Icon::SquareMinus => "square-minus",
            Icon::SquarePlus => "square-plus",
            Icon::Trash => "trash",
            Icon::ListCollapse => "list-chevrons-down-up",
            Icon::ListExpand => "list-chevrons-up-down",
            Icon::Pencil => "pencil",
            Icon::Menu => "menu",
            Icon::Moon => "moon",
            Icon::AlignCenterHorizontal => "align-center-horizontal",
            Icon::AlignCenterVertical => "align-center-vertical",
            Icon::AlignEndHorizontal => "align-end-horizontal",
            Icon::AlignHorizontalDistributeCenter => "align-horizontal-distribute-center",
            Icon::AlignHorizontalDistributeEnd => "align-horizontal-distribute-end",
            Icon::AlignHorizontalDistributeStart => "align-horizontal-distribute-start",
            Icon::AlignHorizontalJustifyCenter => "align-horizontal-justify-center",
            Icon::AlignHorizontalJustifyEnd => "align-horizontal-justify-end",
            Icon::AlignHorizontalJustifyStart => "align-horizontal-justify-start",
            Icon::AlignHorizontalSpaceAround => "align-horizontal-space-around",
            Icon::AlignHorizontalSpaceBetween => "align-horizontal-space-between",
            Icon::AlignStartHorizontal => "align-start-horizontal",
            Icon::AlignStartVertical => "align-start-vertical",
            Icon::AlignVerticalDistributeCenter => "align-vertical-distribute-center",
            Icon::AlignVerticalDistributeEnd => "align-vertical-distribute-end",
            Icon::AlignVerticalDistributeStart => "align-vertical-distribute-start",
            Icon::AlignVerticalJustifyCenter => "align-vertical-justify-center",
            Icon::AlignVerticalJustifyEnd => "align-vertical-justify-end",
            Icon::AlignVerticalJustifyStart => "align-vertical-justify-start",
            Icon::AlignVerticalSpaceAround => "align-vertical-space-around",
            Icon::AlignVerticalSpaceBetween => "align-vertical-space-between",
            Icon::ArrowDownAZ => "arrow-down-a-z",
            Icon::ArrowDownZA => "arrow-down-z-a",
            Icon::ArrowUpAZ => "arrow-up-a-z",
            Icon::BetweenHorizontalEnd => "between-horizontal-end",
            Icon::BetweenHorizontalStart => "between-horizontal-start",
            Icon::BetweenVerticalEnd => "between-vertical-end",
            Icon::BetweenVerticalStart => "between-vertical-start",
            Icon::Cable => "cable",
            Icon::ChevronsDownUp => "chevrons-down-up",
            Icon::ChevronsLeftRightEllipsis => "chevrons-left-right-ellipsis",
            Icon::ChevronsUp => "chevrons-up",
            Icon::ChevronsUpDown => "chevrons-up-down",
            Icon::CircleX => "circle-x",
            Icon::Eye => "eye",
            Icon::EyeOff => "eye-off",
            Icon::Frame => "frame",
            Icon::Funnel => "funnel",
            Icon::FunnelX => "funnel-x",
            Icon::GripVertical => "grip-vertical",
            Icon::Info => "info",
            Icon::MousePointer2 => "mouse-pointer-2",
            Icon::PackageCheck => "package-check",
            Icon::PackageOpen => "package-open",
            Icon::PanelTop => "panel-top",
            Icon::Radar => "radar",
            Icon::Save => "save",
            Icon::SaveCheck => "save-check",
            Icon::SavePen => "save-pen",
            Icon::Scan => "scan",
            Icon::SlidersHorizontal => "sliders-horizontal",
            Icon::Square => "square",
            Icon::SquareDashedTopSolid => "square-dashed-top-solid",
            Icon::SquareMenu => "square-menu",
            Icon::Squircle => "squircle",
            Icon::SquircleDashed => "squircle-dashed",
            Icon::Sun => "sun",
            Icon::Tab => "tab",
            Icon::TabText => "tab-text",
            Icon::TabX => "tab-x",
            Icon::TagPlus => "tag-plus",
            Icon::VectorSquare => "vector-square",
            Icon::DoorClosed => "door-closed",
            Icon::StickyNote => "sticky-note",
            Icon::List => "list",
            Icon::Braces => "braces",
            Icon::Workflow => "workflow",
            Icon::Activity => "activity",
            Icon::ArrowLeftRight => "arrow-left-right",
            Icon::Folder => "folder",
            Icon::FolderClosed => "folder-closed",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_all_has_88_entries() {
        assert_eq!(Icon::ALL.len(), 88);
    }

    #[test]
    fn icon_all_is_in_field_order_at_the_edges() {
        assert_eq!(Icon::ALL[0], Icon::Package);
        assert_eq!(Icon::ALL[1], Icon::Message);
        assert_eq!(Icon::ALL[85], Icon::ArrowLeftRight);
        assert_eq!(Icon::ALL[86], Icon::Folder);
        assert_eq!(Icon::ALL[87], Icon::FolderClosed);
    }

    #[test]
    fn icon_labels_are_unique_and_nonempty() {
        use std::collections::HashSet;
        let mut seen = HashSet::new();
        for icon in Icon::ALL {
            let l = icon.label();
            assert!(!l.is_empty(), "empty label for {icon:?}");
            assert!(seen.insert(l), "duplicate label {l:?}");
        }
        assert_eq!(seen.len(), 88);
    }

    #[test]
    fn label_reflects_lucide_slugs_not_field_names() {
        // Slugs diverge from field names for the hand-named glyphs.
        assert_eq!(Icon::Message.label(), "message-square-text");
        assert_eq!(Icon::Paintbrush.label(), "paintbrush-vertical");
        assert_eq!(Icon::ListCollapse.label(), "list-chevrons-down-up");
        assert_eq!(Icon::ListExpand.label(), "list-chevrons-up-down");
    }
}
