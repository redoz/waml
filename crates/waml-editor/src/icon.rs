//! Generic icon abstraction: a wedge/tool references an `Icon` without knowing
//! how it paints. Not a font atlas -- `Shape` icons are shader-drawn SDFs on a
//! `DrawColor` (`mod.draw.DrawIcon`), matching the mock's hand-drawn glyphs.
//! `Glyph(char)` keeps the existing single-char `DrawText` path valid for
//! callers that have no SDF shape yet. Grows one branch at a time.

use makepad_widgets::*;

/// A drawable icon. Additive: `Texture(TextureId)` can be added later with no
/// API break.
///
/// First landing unit: no Rust caller yet -- the `Radial` widget (a later
/// task) is the consumer. Allowed dead until then, same convention as
/// `icons::TreeIcons::labeled_mut` / `card::Font`.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub enum Icon {
    /// Single char drawn by the caller's own `DrawText` pen (placeholder path).
    Glyph(char),
    /// Shader-drawn SDF selected by `IconShape`.
    Shape(IconShape),
}

/// The seed SDF set: exactly the four node-radial commands. Adding an icon =
/// one variant here + one `pixel()` branch in `mod.draw.DrawIcon`.
///
/// First landing unit: no Rust caller yet -- see `Icon`'s doc comment.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconShape {
    Open,
    Style,
    Markdown,
    Remove,
    // Logo-radial glyphs (Properties/About/Cancel/Exit).
    Properties,
    About,
    Cancel,
    Exit,
}

impl IconShape {
    /// The `shape` uniform value the `DrawIcon` shader switches on. Dense and
    /// stable -- do not renumber existing variants.
    #[allow(dead_code)]
    pub fn shader_index(self) -> u32 {
        match self {
            IconShape::Open => 0,
            IconShape::Style => 1,
            IconShape::Markdown => 2,
            IconShape::Remove => 3,
            IconShape::Properties => 4,
            IconShape::About => 5,
            IconShape::Cancel => 6,
            IconShape::Exit => 7,
        }
    }
}

impl Icon {
    /// The char for a `Glyph` icon (caller draws it with its own `DrawText`);
    /// `None` for `Shape` icons.
    #[allow(dead_code)]
    pub fn glyph(&self) -> Option<char> {
        match self {
            Icon::Glyph(c) => Some(*c),
            Icon::Shape(_) => None,
        }
    }
}

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas

    // One `DrawColor` whose `pixel()` switches on the `shape` uniform (set per
    // draw via `set_uniform`, the proven `draw_node`/`zoom` pattern). The tint
    // is chosen INSIDE the shader from `atlas.accent`/`danger`/`text_dim`
    // (never a hardcoded RGBA from Rust): `danger`=1 uses the danger token,
    // `enabled`=0 forces the dim token. Sharp shapes use `sdf.rect`/`sdf.circle`
    // with a safe inner margin -- path strokes near the viewport edge degenerate
    // silently (repo memory). Branches use MPSL `if` (proven in the fork's
    // draw_glyph pixel shader).
    mod.draw.DrawIcon = mod.draw.DrawColor{
        accent: uniform(atlas.accent)
        danger_col: uniform(atlas.danger)
        dim_col: uniform(atlas.text_dim)
        shape: uniform(0.0)
        danger: uniform(0.0)
        enabled: uniform(1.0)
        pixel: fn() {
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            let s = self.rect_size.x
            let m = s * 0.22
            let hue = mix(self.accent, self.danger_col, self.danger)
            let col = mix(self.dim_col, hue, self.enabled)
            if self.shape < 0.5 {
                // Open: a card outline (screenshot-tuned later).
                sdf.rect(m, m, s - m * 2.0, s - m * 2.0)
                sdf.stroke(col, 1.5)
            } else if self.shape < 1.5 {
                // Style: a filled disc (swatch) -- tuned later.
                sdf.circle(s * 0.5, s * 0.5, s * 0.24)
                sdf.fill(col)
            } else if self.shape < 2.5 {
                // Markdown: three stacked bars.
                sdf.rect(m, s * 0.34, s - m * 2.0, s * 0.06)
                sdf.fill(col)
                sdf.rect(m, s * 0.48, s - m * 2.0, s * 0.06)
                sdf.fill(col)
                sdf.rect(m, s * 0.62, s - m * 2.0, s * 0.06)
                sdf.fill(col)
            } else if self.shape < 3.5 {
                // Remove: an X built from two short segments (kept off the edge).
                sdf.move_to(m, m)
                sdf.line_to(s - m, s - m)
                sdf.stroke(col, 1.8)
                sdf.move_to(s - m, m)
                sdf.line_to(m, s - m)
                sdf.stroke(col, 1.8)
            } else if self.shape < 4.5 {
                // Properties: three horizontal sliders with offset knobs.
                sdf.rect(m, s * 0.30, s - m * 2.0, s * 0.05)
                sdf.fill(col)
                sdf.circle(s * 0.62, s * 0.325, s * 0.075)
                sdf.fill(col)
                sdf.rect(m, s * 0.475, s - m * 2.0, s * 0.05)
                sdf.fill(col)
                sdf.circle(s * 0.40, s * 0.50, s * 0.075)
                sdf.fill(col)
                sdf.rect(m, s * 0.65, s - m * 2.0, s * 0.05)
                sdf.fill(col)
                sdf.circle(s * 0.70, s * 0.675, s * 0.075)
                sdf.fill(col)
            } else if self.shape < 5.5 {
                // About: info "i" -- a dot above a short vertical stem.
                sdf.circle(s * 0.5, s * 0.30, s * 0.06)
                sdf.fill(col)
                sdf.rect(s * 0.5 - s * 0.045, s * 0.42, s * 0.09, s * 0.30)
                sdf.fill(col)
            } else if self.shape < 6.5 {
                // Cancel: an X (same construction as Remove).
                sdf.move_to(m, m)
                sdf.line_to(s - m, s - m)
                sdf.stroke(col, 1.8)
                sdf.move_to(s - m, m)
                sdf.line_to(m, s - m)
                sdf.stroke(col, 1.8)
            } else {
                // Exit: power glyph -- a ring with a top stem breaking into it.
                sdf.circle(s * 0.5, s * 0.55, s * 0.24)
                sdf.stroke(col, 1.8)
                sdf.rect(s * 0.5 - s * 0.03, m, s * 0.06, s * 0.30)
                sdf.fill(col)
            }
            return sdf.result
        }
    }
}

/// Draw a `Shape` icon into `rect`. The hue is chosen inside the shader from
/// the atlas tokens via the `danger`/`enabled` flags (no color crosses Rust).
/// `Glyph` icons are a no-op here -- the caller draws them with its own
/// `DrawText` pen. Returns `true` if this drew (i.e. it was a `Shape`).
///
/// First landing unit: no Rust caller yet -- see `Icon`'s doc comment.
#[allow(dead_code)]
pub fn draw_icon(
    cx: &mut Cx2d,
    draw: &mut DrawColor,
    rect: Rect,
    icon: &Icon,
    danger: bool,
    enabled: bool,
) -> bool {
    match icon {
        Icon::Shape(shape) => {
            draw.set_uniform(cx, live_id!(shape), &[shape.shader_index() as f32]);
            draw.set_uniform(cx, live_id!(danger), &[if danger { 1.0 } else { 0.0 }]);
            draw.set_uniform(cx, live_id!(enabled), &[if enabled { 1.0 } else { 0.0 }]);
            draw.draw_abs(cx, rect);
            true
        }
        Icon::Glyph(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shader_index_is_stable_and_dense() {
        assert_eq!(IconShape::Open.shader_index(), 0);
        assert_eq!(IconShape::Style.shader_index(), 1);
        assert_eq!(IconShape::Markdown.shader_index(), 2);
        assert_eq!(IconShape::Remove.shader_index(), 3);
        assert_eq!(IconShape::Properties.shader_index(), 4);
        assert_eq!(IconShape::About.shader_index(), 5);
        assert_eq!(IconShape::Cancel.shader_index(), 6);
        assert_eq!(IconShape::Exit.shader_index(), 7);
    }

    #[test]
    fn glyph_accessor_only_returns_for_glyph_variant() {
        assert_eq!(Icon::Glyph('H').glyph(), Some('H'));
        assert_eq!(Icon::Shape(IconShape::Open).glyph(), None);
    }
}
