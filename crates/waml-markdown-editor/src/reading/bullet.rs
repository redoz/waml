//! The list-item marker of the reading view, drawn as a DECORATION.
//!
//! A bullet is never substitute text. A glyph backed by no source range would
//! break the invariant that everything drawn maps back to source, so the
//! marker is a shape the viewer draws into the gutter `TextFlow` reserved.
//!
//! Shape varies with nesting depth, which is what makes a nested list legible:
//! disc, then ring, then square, cycling.

use makepad_widgets::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BulletShape {
    Disc,
    Ring,
    Square,
}

/// The shape for a list item at nesting depth `level`.
pub fn bullet_shape_for_level(level: u8) -> BulletShape {
    match level % 3 {
        0 => BulletShape::Disc,
        1 => BulletShape::Ring,
        _ => BulletShape::Square,
    }
}

impl BulletShape {
    /// The `shape` uniform the shader switches on.
    pub fn shader_index(self) -> f32 {
        match self {
            Self::Disc => 0.0,
            Self::Ring => 1.0,
            Self::Square => 2.0,
        }
    }
}

script_mod! {
    use mod.prelude.widgets_internal.*

    // List-item bullet pen, its own shader type so `shape` and `color` ride
    // per-draw (see `DrawReadingBullet`), letting each nesting level and theme
    // color come from one instance. `sdf.box(.., 0)` floods the quad in this
    // fork -- use `sdf.rect` for the square case rather than a zero-radius box.
    set_type_default() do #(DrawReadingBullet::script_shader(vm)){
        ..mod.draw.DrawQuad
        pixel: fn() {
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            let r = min(self.rect_size.x, self.rect_size.y) * 0.5
            let c = self.rect_size * 0.5
            if self.shape < 0.5 {
                sdf.circle(c.x, c.y, r)
                sdf.fill(self.color)
            } else if self.shape < 1.5 {
                sdf.circle(c.x, c.y, r)
                sdf.stroke(self.color, max(1.0, r * 0.4))
            } else {
                sdf.rect(c.x - r, c.y - r, r * 2.0, r * 2.0)
                sdf.fill(self.color)
            }
            return sdf.result
        }
    }
}

#[derive(Script, ScriptHook)]
#[repr(C)]
pub struct DrawReadingBullet {
    #[deref]
    pub draw_super: DrawQuad,
    /// 0 = disc, 1 = ring, 2 = square. See `BulletShape::shader_index`.
    #[live(0.0)]
    pub shape: f32,
    #[live]
    pub color: Vec4f,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nesting_depth_cycles_the_bullet_shape() {
        assert_eq!(bullet_shape_for_level(0), BulletShape::Disc);
        assert_eq!(bullet_shape_for_level(1), BulletShape::Ring);
        assert_eq!(bullet_shape_for_level(2), BulletShape::Square);
        assert_eq!(
            bullet_shape_for_level(3),
            BulletShape::Disc,
            "deep nesting cycles rather than running out of shapes"
        );
    }

    #[test]
    fn every_shape_has_a_distinct_shader_index() {
        let mut indices = [
            BulletShape::Disc.shader_index(),
            BulletShape::Ring.shader_index(),
            BulletShape::Square.shader_index(),
        ];
        indices.sort_by(f32::total_cmp);
        assert_eq!(indices, [0.0, 1.0, 2.0]);
    }
}
