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
    // Faithful port of resources/icons/message-square-text.svg via scripts/
    // gen-icon.py (the bubble outline + tail are the source's `a` arcs, one
    // continuous path, not sdf.box plus a separate wedge).
    // Not yet mapped to a kind -- authored ahead for later (e.g. comments/chat).
    mod.draw.IconMessage = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.9277, s * 0.7138)
            sdf.arc_to(s * 0.8421, s * 0.7138, s * 0.0855, 0.0000, 1.5708)
            sdf.line_to(s * 0.2788, s * 0.7994)
            sdf.arc_to(s * 0.2788, s * 0.8849, s * 0.0855, -1.5710, -2.3563)
            sdf.line_to(s * 0.1242, s * 0.9186)
            sdf.arc_to(s * 0.1027, s * 0.8971, s * 0.0304, 0.7855, 3.1415)
            sdf.line_to(s * 0.0723, s * 0.2006)
            sdf.arc_to(s * 0.1579, s * 0.2006, s * 0.0855, 3.1416, 4.7124)
            sdf.line_to(s * 0.8421, s * 0.1151)
            sdf.arc_to(s * 0.8421, s * 0.2006, s * 0.0855, -1.5708, 0.0000)
            sdf.line_to(s * 0.9277, s * 0.7138)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2862, s * 0.4572)
            sdf.line_to(s * 0.7138, s * 0.4572)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2862, s * 0.6283)
            sdf.line_to(s * 0.5428, s * 0.6283)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2862, s * 0.2862)
            sdf.line_to(s * 0.6283, s * 0.2862)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Package+: the package cube with a plus badge (add-to-package action).
    // Faithful port of resources/icons/package-plus.svg via scripts/gen-icon.py
    // (the box seams are the source's `a`-cornered hull, the plus its two lines).
    mod.draw.IconPackagePlus = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.5000, s * 0.9277)
            sdf.line_to(s * 0.5000, s * 0.5000)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6711, s * 0.7138)
            sdf.line_to(s * 0.9277, s * 0.7138)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.7994, s * 0.5855)
            sdf.line_to(s * 0.7994, s * 0.8421)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.8849, s * 0.4373)
            sdf.line_to(s * 0.8849, s * 0.3289)
            sdf.arc_to(s * 0.7994, s * 0.3290, s * 0.0855, -0.0010, -1.0472)
            sdf.line_to(s * 0.5428, s * 0.0839)
            sdf.arc_to(s * 0.5000, s * 0.1579, s * 0.0855, -1.0472, -2.0944)
            sdf.line_to(s * 0.1579, s * 0.2549)
            sdf.arc_to(s * 0.2006, s * 0.3290, s * 0.0855, -2.0944, -3.1406)
            sdf.line_to(s * 0.1151, s * 0.6711)
            sdf.arc_to(s * 0.2006, s * 0.6709, s * 0.0855, 3.1401, 2.0944)
            sdf.line_to(s * 0.4572, s * 0.9161)
            sdf.arc_to(s * 0.5000, s * 0.8420, s * 0.0855, 2.0949, 1.0477)
            sdf.line_to(s * 0.6144, s * 0.8753)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.1275, s * 0.2862)
            sdf.line_to(s * 0.5000, s * 0.5000)
            sdf.line_to(s * 0.8725, s * 0.2862)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.3075, s * 0.1694)
            sdf.line_to(s * 0.6923, s * 0.3896)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Paintbrush (vertical): two bristle ticks, ferrule box, and brush body.
    // Faithful port of resources/icons/paintbrush-vertical.svg via scripts/
    // gen-icon.py (ferrule + brush corners are the source's `a` fillets).
    mod.draw.IconPaintbrush = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.4145, s * 0.0723)
            sdf.line_to(s * 0.4145, s * 0.1579)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.5855, s * 0.0723)
            sdf.line_to(s * 0.5855, s * 0.2434)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.7138, s * 0.0723)
            sdf.arc_to(s * 0.7138, s * 0.1151, s * 0.0428, -1.5708, 0.0000)
            sdf.line_to(s * 0.7566, s * 0.5000)
            sdf.line_to(s * 0.2434, s * 0.5000)
            sdf.line_to(s * 0.2434, s * 0.1151)
            sdf.arc_to(s * 0.2862, s * 0.1151, s * 0.0428, 3.1416, 4.7124)
            sdf.line_to(s * 0.7138, s * 0.0723)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2434, s * 0.5000)
            sdf.arc_to(s * 0.2434, s * 0.5428, s * 0.0428, -1.5708, -3.1416)
            sdf.line_to(s * 0.2006, s * 0.5855)
            sdf.arc_to(s * 0.2862, s * 0.5855, s * 0.0855, 3.1416, 1.5708)
            sdf.line_to(s * 0.3717, s * 0.6711)
            sdf.arc_to(s * 0.3717, s * 0.7138, s * 0.0428, -1.5708, 0.0000)
            sdf.line_to(s * 0.4145, s * 0.8379)
            sdf.arc_to(s * 0.5000, s * 0.8379, s * 0.0855, 3.1416, 0.0000)
            sdf.line_to(s * 0.5855, s * 0.7138)
            sdf.arc_to(s * 0.6283, s * 0.7138, s * 0.0428, 3.1416, 4.7124)
            sdf.line_to(s * 0.7138, s * 0.6711)
            sdf.arc_to(s * 0.7138, s * 0.5855, s * 0.0855, 1.5708, 0.0000)
            sdf.line_to(s * 0.7994, s * 0.5428)
            sdf.arc_to(s * 0.7566, s * 0.5428, s * 0.0428, 0.0000, -1.5708)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Pin: the Lucide pushpin, a faithful port of resources/icons/pin.svg.
    // Straight runs are line_to; the seven rounded fillets + the cap are the
    // SVG's circular `a` arcs, expressed directly as sdf.arc_to centerline
    // segments (no hand-flattened polyline). Every arc is rx==ry, so a single
    // radius suffices. The endpoint->center conversion and the cell fit are done
    // offline by scripts/gen-pin-icon.py (norm(c) = 0.042768*c - 0.013216, uniform
    // + isotropic so arc angles pass through unchanged); regenerate this body by
    // rerunning it. The fit holds the stroke inside the
    // cell (Sdf2d.viewport clips at rect_size, unlike DrawSvg which bleeds its
    // stroke outside). Sdf2d.stroke(w) treats w as the HALF-width.
    mod.draw.IconPin = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.3717, s * 0.4470)
            sdf.arc_to(s * 0.2862, s * 0.4469, s * 0.0855, 0.0005, 1.1096)
            sdf.line_to(s * 0.2481, s * 0.5620)
            sdf.arc_to(s * 0.2862, s * 0.6386, s * 0.0855, -2.0320, -3.1411)
            sdf.line_to(s * 0.2006, s * 0.6711)
            sdf.arc_to(s * 0.2434, s * 0.6711, s * 0.0428, 3.1416, 1.5708)
            sdf.line_to(s * 0.7566, s * 0.7138)
            sdf.arc_to(s * 0.7566, s * 0.6711, s * 0.0428, 1.5708, 0.0000)
            sdf.line_to(s * 0.7994, s * 0.6386)
            sdf.arc_to(s * 0.7138, s * 0.6386, s * 0.0855, -0.0005, -1.1096)
            sdf.line_to(s * 0.6758, s * 0.5235)
            sdf.arc_to(s * 0.7138, s * 0.4469, s * 0.0855, 2.0320, 3.1411)
            sdf.line_to(s * 0.6283, s * 0.2862)
            sdf.arc_to(s * 0.6711, s * 0.2862, s * 0.0428, 3.1416, 4.7124)
            sdf.arc_to(s * 0.6711, s * 0.1579, s * 0.0855, 1.5708, -1.5708)
            sdf.line_to(s * 0.3289, s * 0.0723)
            sdf.arc_to(s * 0.3289, s * 0.1579, s * 0.0855, -1.5708, -4.7124)
            sdf.arc_to(s * 0.3289, s * 0.2862, s * 0.0428, -1.5708, 0.0000)
            sdf.line_to(s * 0.3717, s * 0.4470)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.5000, s * 0.7138)
            sdf.line_to(s * 0.5000, s * 0.9277)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Pin-off: the pushpin outline plus a corner-to-corner strike (unpinned).
    mod.draw.IconPinOff = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.085
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.33, s * 0.12)
            sdf.line_to(s * 0.67, s * 0.12)
            sdf.line_to(s * 0.67, s * 0.27)
            sdf.line_to(s * 0.625, s * 0.30)
            sdf.line_to(s * 0.625, s * 0.45)
            sdf.line_to(s * 0.792, s * 0.63)
            sdf.line_to(s * 0.792, s * 0.66)
            sdf.line_to(s * 0.75, s * 0.69)
            sdf.line_to(s * 0.25, s * 0.69)
            sdf.line_to(s * 0.208, s * 0.66)
            sdf.line_to(s * 0.208, s * 0.63)
            sdf.line_to(s * 0.375, s * 0.45)
            sdf.line_to(s * 0.375, s * 0.30)
            sdf.line_to(s * 0.33, s * 0.27)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.5, s * 0.69)
            sdf.line_to(s * 0.5, s * 0.89)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.14, s * 0.14)
            sdf.line_to(s * 0.86, s * 0.86)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Share: open tray + up-and-out arrow. Faithful port of resources/icons/
    // share.svg via scripts/gen-icon.py (shared Lucide fit; tray fillets are the
    // svg's circular `a` arcs, not squared corners).
    mod.draw.IconShare = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.5000, s * 0.0723)
            sdf.line_to(s * 0.5000, s * 0.6283)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6711, s * 0.2434)
            sdf.line_to(s * 0.5000, s * 0.0723)
            sdf.line_to(s * 0.3289, s * 0.2434)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.1579, s * 0.5000)
            sdf.line_to(s * 0.1579, s * 0.8421)
            sdf.arc_to(s * 0.2434, s * 0.8421, s * 0.0855, 3.1416, 1.5708)
            sdf.line_to(s * 0.7566, s * 0.9277)
            sdf.arc_to(s * 0.7566, s * 0.8421, s * 0.0855, 1.5708, 0.0000)
            sdf.line_to(s * 0.8421, s * 0.5000)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Spline: two endpoint node rings joined by a true quarter sweep. Faithful
    // port of resources/icons/spline.svg via scripts/gen-icon.py (the joining
    // stroke is the source's circular `A` arc, not a hand-flattened polyline;
    // the nodes are its two <circle> elements as stroked rings).
    mod.draw.IconSpline = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.8849, s * 0.2006)
            sdf.arc_to(s * 0.7994, s * 0.2006, s * 0.0855, 0.0000, 3.1416)
            sdf.arc_to(s * 0.7994, s * 0.2006, s * 0.0855, 3.1416, 6.2832)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2862, s * 0.7994)
            sdf.arc_to(s * 0.2006, s * 0.7994, s * 0.0855, 0.0000, 3.1416)
            sdf.arc_to(s * 0.2006, s * 0.7994, s * 0.0855, 3.1416, 6.2832)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2006, s * 0.7138)
            sdf.arc_to(s * 0.7138, s * 0.7138, s * 0.5132, 3.1416, 4.7124)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Spline-pointer: a spline sweep with a cursor pointer at the tail plus two
    // node rings. Faithful port of resources/icons/spline-pointer.svg via
    // scripts/gen-icon.py (cursor + curve are the source's `a`/`A` arcs, the
    // nodes its two <circle> elements).
    mod.draw.IconSplinePointer = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.5015, s * 0.5291)
            sdf.arc_to(s * 0.5212, s * 0.5212, s * 0.0213, 2.7623, 5.0917)
            sdf.line_to(s * 0.9140, s * 0.6511)
            sdf.arc_to(s * 0.9063, s * 0.6711, s * 0.0214, -1.1983, 1.2683)
            sdf.line_to(s * 0.7653, s * 0.7371)
            sdf.arc_to(s * 0.7780, s * 0.7780, s * 0.0428, -1.8706, -2.8417)
            sdf.line_to(s * 0.6915, s * 0.9126)
            sdf.arc_to(s * 0.6711, s * 0.9063, s * 0.0214, 0.3025, 2.7691)
            sdf.line_to(s * 0.5015, s * 0.5291)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2006, s * 0.7138)
            sdf.arc_to(s * 0.7138, s * 0.7138, s * 0.5132, 3.1416, 4.7124)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.8849, s * 0.2006)
            sdf.arc_to(s * 0.7994, s * 0.2006, s * 0.0855, 0.0000, 3.1416)
            sdf.arc_to(s * 0.7994, s * 0.2006, s * 0.0855, 3.1416, 6.2832)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2862, s * 0.7994)
            sdf.arc_to(s * 0.2006, s * 0.7994, s * 0.0855, 0.0000, 3.1416)
            sdf.arc_to(s * 0.2006, s * 0.7994, s * 0.0855, 3.1416, 6.2832)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Square-minus: rounded square with a minus. Faithful port of resources/
    // icons/square-minus.svg via scripts/gen-icon.py (the <rect rx="2"> box is
    // authored as its four corner `a` arcs, not sdf.box).
    mod.draw.IconSquareMinus = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.2006, s * 0.1151)
            sdf.line_to(s * 0.7994, s * 0.1151)
            sdf.arc_to(s * 0.7994, s * 0.2006, s * 0.0855, -1.5708, 0.0000)
            sdf.line_to(s * 0.8849, s * 0.7994)
            sdf.arc_to(s * 0.7994, s * 0.7994, s * 0.0855, 0.0000, 1.5708)
            sdf.line_to(s * 0.2006, s * 0.8849)
            sdf.arc_to(s * 0.2006, s * 0.7994, s * 0.0855, 1.5708, 3.1416)
            sdf.line_to(s * 0.1151, s * 0.2006)
            sdf.arc_to(s * 0.2006, s * 0.2006, s * 0.0855, 3.1416, 4.7124)
            sdf.line_to(s * 0.2006, s * 0.1151)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.3289, s * 0.5000)
            sdf.line_to(s * 0.6711, s * 0.5000)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Square-plus: rounded square with a plus. Faithful port of resources/icons/
    // square-plus.svg via scripts/gen-icon.py (the <rect rx="2"> box is its four
    // corner `a` arcs, not sdf.box).
    mod.draw.IconSquarePlus = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.2006, s * 0.1151)
            sdf.line_to(s * 0.7994, s * 0.1151)
            sdf.arc_to(s * 0.7994, s * 0.2006, s * 0.0855, -1.5708, 0.0000)
            sdf.line_to(s * 0.8849, s * 0.7994)
            sdf.arc_to(s * 0.7994, s * 0.7994, s * 0.0855, 0.0000, 1.5708)
            sdf.line_to(s * 0.2006, s * 0.8849)
            sdf.arc_to(s * 0.2006, s * 0.7994, s * 0.0855, 1.5708, 3.1416)
            sdf.line_to(s * 0.1151, s * 0.2006)
            sdf.arc_to(s * 0.2006, s * 0.2006, s * 0.0855, 3.1416, 4.7124)
            sdf.line_to(s * 0.2006, s * 0.1151)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.3289, s * 0.5000)
            sdf.line_to(s * 0.6711, s * 0.5000)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.5000, s * 0.3289)
            sdf.line_to(s * 0.5000, s * 0.6711)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Trash: can body, lid line, and handle. Faithful port of resources/icons/
    // trash.svg via scripts/gen-icon.py (can-bottom and handle corners are the
    // source's rounded `a` fillets, not squared).
    mod.draw.IconTrash = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.7994, s * 0.2434)
            sdf.line_to(s * 0.7994, s * 0.8421)
            sdf.arc_to(s * 0.7138, s * 0.8421, s * 0.0855, 0.0000, 1.5708)
            sdf.line_to(s * 0.2862, s * 0.9277)
            sdf.arc_to(s * 0.2862, s * 0.8421, s * 0.0855, 1.5708, 3.1416)
            sdf.line_to(s * 0.2006, s * 0.2434)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.1151, s * 0.2434)
            sdf.line_to(s * 0.8849, s * 0.2434)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.3289, s * 0.2434)
            sdf.line_to(s * 0.3289, s * 0.1579)
            sdf.arc_to(s * 0.4145, s * 0.1579, s * 0.0855, 3.1416, 4.7124)
            sdf.line_to(s * 0.5855, s * 0.0723)
            sdf.arc_to(s * 0.5855, s * 0.1579, s * 0.0855, -1.5708, 0.0000)
            sdf.line_to(s * 0.6711, s * 0.2434)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // List collapse (down-up): three rows + chevrons pointing inward.
    // Faithful port of resources/icons/list-chevrons-down-up.svg via
    // scripts/gen-icon.py; scale nudged up (A=0.048, B=-0.076) to sit on the
    // SVG in the harness, center held at c=12.
    mod.draw.IconListCollapse = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.0680, s * 0.1640)
            sdf.line_to(s * 0.4520, s * 0.1640)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.0680, s * 0.5000)
            sdf.line_to(s * 0.4520, s * 0.5000)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.0680, s * 0.8360)
            sdf.line_to(s * 0.4520, s * 0.8360)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6440, s * 0.1640)
            sdf.line_to(s * 0.7880, s * 0.3080)
            sdf.line_to(s * 0.9320, s * 0.1640)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6440, s * 0.8360)
            sdf.line_to(s * 0.7880, s * 0.6920)
            sdf.line_to(s * 0.9320, s * 0.8360)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // List expand (up-down): three rows + chevrons pointing outward.
    // Faithful port of resources/icons/list-chevrons-up-down.svg via
    // scripts/gen-icon.py; scale nudged up (A=0.048, B=-0.076) to match its
    // mirror sibling IconListCollapse, center held at c=12.
    mod.draw.IconListExpand = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.0680, s * 0.1640)
            sdf.line_to(s * 0.4520, s * 0.1640)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.0680, s * 0.5000)
            sdf.line_to(s * 0.4520, s * 0.5000)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.0680, s * 0.8360)
            sdf.line_to(s * 0.4520, s * 0.8360)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6440, s * 0.3080)
            sdf.line_to(s * 0.7880, s * 0.1640)
            sdf.line_to(s * 0.9320, s * 0.3080)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6440, s * 0.6920)
            sdf.line_to(s * 0.7880, s * 0.8360)
            sdf.line_to(s * 0.9320, s * 0.6920)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Pencil: eraser-diamond body tapering to a tip, plus the ferrule crease.
    // Faithful port of resources/icons/pencil.svg via scripts/gen-icon.py (the
    // rounded body corners are the source's `a` fillets).
    mod.draw.IconPencil = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.8924, s * 0.2781)
            sdf.arc_to(s * 0.8071, s * 0.1929, s * 0.1206, 0.7855, -2.3561)
            sdf.line_to(s * 0.1511, s * 0.6785)
            sdf.arc_to(s * 0.2115, s * 0.7391, s * 0.0855, -2.3547, -2.8441)
            sdf.line_to(s * 0.0732, s * 0.9001)
            sdf.arc_to(s * 0.0937, s * 0.9063, s * 0.0214, -2.8512, -5.0044)
            sdf.line_to(s * 0.2860, s * 0.8703)
            sdf.arc_to(s * 0.2611, s * 0.7885, s * 0.0855, 1.2755, 0.7870)
            sdf.line_to(s * 0.8924, s * 0.2781)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.6283, s * 0.2006)
            sdf.line_to(s * 0.7994, s * 0.3717)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Menu (hamburger): three full-width rows.
    // Faithful port of resources/icons/menu.svg via scripts/gen-icon.py; scale
    // nudged up (A=0.055, B=-0.16) to fill the cell past menu.svg's baked
    // viewBox padding, center held at c=12.
    mod.draw.IconMenu = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.0600, s * 0.1150)
            sdf.line_to(s * 0.9400, s * 0.1150)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.0600, s * 0.5000)
            sdf.line_to(s * 0.9400, s * 0.5000)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.0600, s * 0.8850)
            sdf.line_to(s * 0.9400, s * 0.8850)
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
}

impl TreeIcons {
    /// All nine glyphs paired with a short label, in a stable order. Used by the
    /// `icon_harness` bin's proof-grid; the shipping tree/doc-tabs pick glyphs by
    /// `TreeKind` via `icon_for` in `tree_panel.rs` instead.
    #[allow(dead_code)]
    pub fn labeled_mut(&mut self) -> [(&'static str, &mut DrawColor); 24] {
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
            ("menu", &mut self.menu),
        ]
    }
}
