//! `NodeDesignEditor`: a Turtle-layout port of the HUD "node design editor" mock
//! (`docs/design/hud-node-design-mock.html`). A frosted Atlas HUD pane
//! (AccentFrame material) carrying an identity header and a two-pane body: a LIVE
//! node preview on an inset canvas (left) that reacts to every control on the
//! right. Compiled into the crate but NOT mounted in the live app -- viewable only
//! via `bin/node_editor_harness.rs`.
//!
//! Rebuilt on makepad's NATIVE layout engine (the first cut hand-positioned every
//! rect with `draw_abs` and guessed text widths with a mono-advance constant --
//! that structural approach is gone). Every box here is a `script_mod!` `View`
//! with `flow`/`Fill`/`Fit`/`spacing`/`padding`, and every glyph is a real `Label`
//! that measures itself. Material lives on each View's own `draw_bg` (a `+:`
//! merge that adds an `accent` uniform + a `pixel` shader, the start-screen idiom)
//! -- the glass frame, toggle knob, swatch fills, chip tints, identity/grip glyphs
//! -- pushed the accent via `set_uniform` per draw so the whole surface recolours
//! when a swatch is picked. Structure is the Turtle's; material is the shader's.
//! (The lone exception, port nubs, straddle the node's border like the mock's
//! `position:absolute` `.ports i` dots, so they draw as a material overlay off the
//! node's measured area rect via the widget-owned `draw_nub` -- material, not
//! layout.)
//!
//! Interaction mirrors `recent_row.rs`' hybrid `#[deref] View`: the widget records
//! each interactive child's *laid-out* area rect during `draw_walk` (never a
//! hand-computed rect) and matches `FingerUp` against them, mutating widget-local
//! `#[rust]` state; there is no Model wiring -- this is a standalone design surface.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.draw
    use mod.widgets.*
    use mod.text.*

    // The HUD glass frame as a standalone draw template, for the widget-owned
    // `draw_nub` field (the port-nub overlay). Derives both border stops from a
    // single `accent` uniform pushed per draw (bright top-left -> dim bottom-right
    // along the 150deg diagonal); sharp `sdf.rect` per the fork's 0-radius
    // `sdf.box` flood gotcha. The View-tree frames carry the same math inline on
    // their own `draw_bg` (see `NdeFrameBox`).
    mod.draw.NdeFrame = mod.draw.DrawColor{
        accent: uniform(atlas.accent)
        pixel: fn() {
            let inset = 1.5
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.rect(inset, inset, self.rect_size.x - inset * 2.0, self.rect_size.y - inset * 2.0)
            sdf.fill_keep(self.color)
            let hi = vec4(self.accent.x, self.accent.y, self.accent.z, 0.95)
            let lo = vec4(self.accent.x, self.accent.y, self.accent.z, 0.50)
            let dir = vec2(0.5, 0.8660254)
            let t = clamp((self.pos.x * dir.x + self.pos.y * dir.y) / 1.3660254, 0.0, 1.0)
            sdf.stroke(mix(hi, lo, t), inset)
            return sdf.result
        }
    }

    // ---- material View templates (per-pixel only; the Turtle owns placement) ----

    // Glass frame surface: flat `color` fill ringed by the accent stroke.
    mod.widgets.NdeFrameBox = View{
        show_bg: true
        draw_bg +: {
            color: #xffffff
            accent: uniform(atlas.accent)
            pixel: fn() {
                let inset = 1.5
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.rect(inset, inset, self.rect_size.x - inset * 2.0, self.rect_size.y - inset * 2.0)
                sdf.fill_keep(self.color)
                let hi = vec4(self.accent.x, self.accent.y, self.accent.z, 0.95)
                let lo = vec4(self.accent.x, self.accent.y, self.accent.z, 0.50)
                let dir = vec2(0.5, 0.8660254)
                let t = clamp((self.pos.x * dir.x + self.pos.y * dir.y) / 1.3660254, 0.0, 1.0)
                sdf.stroke(mix(hi, lo, t), inset)
                return sdf.result
            }
        }
    }

    // Flat accent wash / hairline: whole rect filled premultiplied at alpha `a`
    // (low-alpha reads as a wash over the white surface, not a bloom -- the
    // start-screen CardShadow / recent-row hover premult convention).
    mod.widgets.NdeWashBox = View{
        show_bg: true
        draw_bg +: {
            accent: uniform(atlas.accent)
            a: uniform(0.14)
            pixel: fn() {
                return vec4(self.accent.x * self.a, self.accent.y * self.a, self.accent.z * self.a, self.a)
            }
        }
    }

    // Accent swatch: fixed-colour rounded chip (`color` per swatch) with a 1.5px
    // accent selection ring when `sel` > 0.5.
    mod.widgets.NdeSwatchBox = View{
        width: 20.0
        height: 20.0
        show_bg: true
        draw_bg +: {
            accent: uniform(atlas.accent)
            sel: uniform(0.0)
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.box(1.5, 1.5, self.rect_size.x - 3.0, self.rect_size.y - 3.0, 5.0)
                sdf.fill(self.color)
                if self.sel > 0.5 {
                    sdf.box(0.75, 0.75, self.rect_size.x - 1.5, self.rect_size.y - 1.5, 5.5)
                    sdf.stroke(vec4(self.accent.x, self.accent.y, self.accent.z, 1.0), 1.5)
                }
                return sdf.result
            }
        }
    }

    // HUD toggle: the whole control in one shader driven by `on` (0 rest / 1 on).
    // Off: white track, accent border, accent-outlined knob left. On: accent
    // track, white knob right. Knob slides via `mix` on `on`.
    mod.widgets.NdeToggleBox = View{
        width: 28.0
        height: 18.0
        show_bg: true
        draw_bg +: {
            accent: uniform(atlas.accent)
            on: uniform(0.0)
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                let w = self.rect_size.x
                let h = self.rect_size.y
                sdf.box(0.5, 0.5, w - 1.0, h - 1.0, 2.0)
                sdf.fill_keep(mix(vec4(1.0, 1.0, 1.0, 1.0), vec4(self.accent.x, self.accent.y, self.accent.z, 1.0), self.on))
                sdf.stroke(vec4(self.accent.x, self.accent.y, self.accent.z, mix(0.35, 1.0, self.on)), 1.0)
                let kx = mix(2.0, w - 16.0, self.on)
                sdf.box(kx, 2.0, 14.0, 14.0, 2.0)
                sdf.fill_keep(mix(vec4(0.0, 0.0, 0.0, 0.0), vec4(1.0, 1.0, 1.0, 1.0), self.on))
                let kb = mix(vec4(self.accent.x, self.accent.y, self.accent.z, 0.55), vec4(1.0, 1.0, 1.0, 1.0), self.on)
                sdf.stroke(kb, 1.0)
                return sdf.result
            }
        }
    }

    // One segmented-control cell: white, or accent-filled when `sel` > 0.5. The
    // group ring / cell text colour come from the parent frame / Rust.
    mod.widgets.NdeSegBox = View{
        show_bg: true
        draw_bg +: {
            accent: uniform(atlas.accent)
            sel: uniform(0.0)
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.rect(0.0, 0.0, self.rect_size.x, self.rect_size.y)
                sdf.fill(mix(vec4(1.0, 1.0, 1.0, 1.0), vec4(self.accent.x, self.accent.y, self.accent.z, 1.0), self.sel))
                return sdf.result
            }
        }
    }

    // Column chip: accent-tint fill when `on` (dimmer when `enabled` is 0), an
    // accent border ring always. Text colour set from Rust. PLACEHOLDER: the
    // locked Name chip's dashed border isn't expressible with a flat stroke --
    // drawn solid.
    mod.widgets.NdeChipBox = View{
        show_bg: true
        draw_bg +: {
            accent: uniform(atlas.accent)
            on: uniform(0.0)
            enabled: uniform(1.0)
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                let w = self.rect_size.x
                let h = self.rect_size.y
                sdf.box(1.0, 1.0, w - 2.0, h - 2.0, 2.0)
                let fa = self.on * mix(0.10, 0.28, self.enabled)
                sdf.fill_keep(vec4(self.accent.x * fa, self.accent.y * fa, self.accent.z * fa, fa))
                let ba = mix(0.30, 0.70, self.on) * mix(0.5, 1.0, self.enabled)
                sdf.stroke(vec4(self.accent.x, self.accent.y, self.accent.z, ba), 1.0)
                return sdf.result
            }
        }
    }

    // Guillemet stereotype chip: an ACTIVE accent chip (these are the allowed
    // stereotypes, so they read live -- a SOLID accent fill with white text, like
    // a selected segment, not a dead grey pill). Text rides on top as a child.
    mod.widgets.NdeTagChipBox = View{
        show_bg: true
        draw_bg +: {
            accent: uniform(atlas.accent)
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.box(0.0, 0.0, self.rect_size.x, self.rect_size.y, 3.0)
                sdf.fill(vec4(self.accent.x, self.accent.y, self.accent.z, 1.0))
                return sdf.result
            }
        }
    }

    // Identity palette glyph: rounded-square outline + 3 dots, accent-tinted.
    mod.widgets.NdeIcoBox = View{
        width: 20.0
        height: 20.0
        show_bg: true
        draw_bg +: {
            accent: uniform(atlas.accent)
            hollow: uniform(atlas.field_bg)
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                let s = self.rect_size
                let a = vec4(self.accent.x, self.accent.y, self.accent.z, 1.0)
                sdf.box(s.x * 0.18, s.y * 0.18, s.x * 0.64, s.y * 0.64, 3.0)
                sdf.fill_keep(self.hollow)
                sdf.stroke(a, 1.6)
                sdf.circle(s.x * 0.38, s.y * 0.38, 1.4)
                sdf.fill(a)
                sdf.circle(s.x * 0.62, s.y * 0.38, 1.4)
                sdf.fill(a)
                sdf.circle(s.x * 0.38, s.y * 0.62, 1.4)
                sdf.fill(a)
                return sdf.result
            }
        }
    }

    // Compartment drag grip: two stacked bars (`col` accent-tinted when the
    // compartment is on, grey when off).
    mod.widgets.NdeGripBox = View{
        width: 16.0
        height: 16.0
        show_bg: true
        draw_bg +: {
            col: uniform(atlas.text_dim)
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                let s = self.rect_size
                sdf.move_to(s.x * 0.22, s.y * 0.40)
                sdf.line_to(s.x * 0.78, s.y * 0.40)
                sdf.stroke(self.col, 1.8)
                sdf.move_to(s.x * 0.22, s.y * 0.62)
                sdf.line_to(s.x * 0.78, s.y * 0.62)
                sdf.stroke(self.col, 1.8)
                return sdf.result
            }
        }
    }

    // ---- styled Label templates (self-measuring text; no advance constants) ----

    mod.widgets.NdeStereo = Label{
        draw_text +: {
            color: atlas.accent
            text_style: TextStyle{
                font_size: 9
                font_family: FontFamily{ latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Mono/IBMPlexMono-Regular.ttf") asc: -0.1 desc: 0.0} }
                line_spacing: 1.2
            }
        }
    }
    // Only IBM Plex Sans Regular ships in resources (no Bold/SemiBold face), so
    // the identity title and section headers use Regular -- a missing face
    // renders NOTHING. Weight can return once the bold faces are added.
    mod.widgets.NdeName = Label{
        draw_text +: {
            color: atlas.text
            text_style: TextStyle{
                font_size: 15
                font_family: FontFamily{ latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0} }
                line_spacing: 1.2
            }
        }
    }
    // Controls: uppercase section header / sub-heading.
    mod.widgets.NdeSection = Label{
        draw_text +: {
            color: atlas.text_dim
            text_style: TextStyle{
                font_size: 10
                font_family: FontFamily{ latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0} }
                line_spacing: 1.2
            }
        }
    }
    // Controls: field labels, chip / segmented text, notes (regular sans). Colour
    // set per use from Rust.
    mod.widgets.NdeCtrl = Label{
        draw_text +: {
            color: atlas.text
            text_style: TextStyle{
                font_size: 11
                font_family: FontFamily{ latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0} }
                line_spacing: 1.0
            }
        }
    }

    // ---- reusable composite templates ----

    mod.widgets.NdeRule = mod.widgets.NdeWashBox{ width: Fill height: 1.0 }
    mod.widgets.NdeToggleView = mod.widgets.NdeToggleBox{}

    mod.widgets.NodeDesignEditorBase = #(NodeDesignEditor::register_widget(vm))

    mod.widgets.NodeDesignEditor = set_type_default() do mod.widgets.NodeDesignEditorBase{
        width: Fill
        height: Fill
        show_bg: false
        flow: Down
        align: Align{x: 0.5, y: 0.0}
        padding: Inset{left: 0.0, right: 0.0, top: 40.0, bottom: 40.0}
        // Port-nub material overlay (drawn abs off the node's measured area).
        draw_nub: mod.draw.NdeFrame{ color: #xffffff }
        // The preview node card is drawn abs off the stagecell area from the
        // shared `card::measure` geometry (the reusable node-render path), NOT a
        // hand-built View subtree. These are the card's own pens.
        draw_frame: mod.draw.NdeFrame{ color: #xffffff }
        draw_rule +: { color: atlas.text_dim }
        draw_mono_bold +: {
            color: atlas.text
            text_style: TextStyle{
                font_size: 14
                font_family: FontFamily{ latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Mono/IBMPlexMono-Bold.ttf") asc: -0.1 desc: 0.0} }
                line_spacing: 1.2
            }
        }
        draw_mono_dim +: {
            color: atlas.text_dim
            text_style: TextStyle{
                font_size: 11
                font_family: FontFamily{ latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Mono/IBMPlexMono-Regular.ttf") asc: -0.1 desc: 0.0} }
                line_spacing: 1.2
            }
        }
        draw_mono_accent +: {
            color: atlas.accent
            text_style: TextStyle{
                font_size: 11
                font_family: FontFamily{ latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Mono/IBMPlexMono-Regular.ttf") asc: -0.1 desc: 0.0} }
                line_spacing: 1.2
            }
        }
        draw_mono_amber +: {
            color: atlas.bucket_amber
            text_style: TextStyle{
                font_size: 11
                font_family: FontFamily{ latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Mono/IBMPlexMono-Regular.ttf") asc: -0.1 desc: 0.0} }
                line_spacing: 1.2
            }
        }

        // The frosted HUD pane. Fixed 660 wide (the mock), height fits its content.
        pane := mod.widgets.NdeFrameBox {
            width: 660.0
            height: Fit
            flow: Down
            draw_bg +: { color: #xffffff }

            // Identity header: palette glyph + «node design» eyebrow + "Entity".
            phead := View {
                width: Fill
                height: Fit
                flow: Right
                align: Align{y: 0.5}
                spacing: 11.0
                padding: Inset{left: 15.0, right: 15.0, top: 13.0, bottom: 13.0}
                ico := mod.widgets.NdeIcoBox {}
                meta := View {
                    width: Fit
                    height: Fit
                    flow: Down
                    spacing: 3.0
                    phead_stereo := mod.widgets.NdeStereo { text: "\u{ab}NODE DESIGN\u{bb}" }
                    phead_name := mod.widgets.NdeName { text: "Entity" }
                }
            }
            phead_rule := mod.widgets.NdeRule { draw_bg +: { a: 0.22 } }

            // Two-pane body: the inset preview canvas | the controls column.
            editor := View {
                width: Fill
                height: Fit
                flow: Right

                // Inset canvas cell (no ground fill -- the node card stands on the
                // pane's own white). The card is drawn abs, centred in this cell,
                // from the shared `card::measure` geometry -- so the cell carries a
                // fixed height rather than hugging a View subtree.
                stagecell := View {
                    width: 258.0
                    height: 280.0
                    flow: Down
                }

                // Right-edge accent hairline between the canvas and the controls.
                body_divider := mod.widgets.NdeWashBox {
                    width: 1.0
                    height: Fill
                    draw_bg +: { a: 0.14 }
                }

                controls := View {
                    width: Fill
                    height: Fit
                    flow: Down

                    // ---- Appearance ----
                    sect_app := View {
                        width: Fill height: Fit flow: Down spacing: 9.0
                        padding: Inset{left: 16.0, right: 16.0, top: 12.0, bottom: 14.0}
                        mod.widgets.NdeSection { text: "APPEARANCE" draw_text +: { color: #x8a97a6 } }
                        app_field := View {
                            width: Fill height: Fit flow: Right align: Align{y: 0.5} spacing: 12.0
                            mod.widgets.NdeCtrl { width: 66.0 text: "Accent" draw_text +: { color: #x6a7686 } }
                            swatches := View {
                                width: Fill height: Fit flow: Right spacing: 6.0 align: Align{y: 0.5}
                                sw0 := mod.widgets.NdeSwatchBox { draw_bg +: { color: #x1496dc } }
                                sw1 := mod.widgets.NdeSwatchBox { draw_bg +: { color: #x00b4d2 } }
                                sw2 := mod.widgets.NdeSwatchBox { draw_bg +: { color: #x14bea0 } }
                                sw3 := mod.widgets.NdeSwatchBox { draw_bg +: { color: #x5a6ef0 } }
                                sw4 := mod.widgets.NdeSwatchBox { draw_bg +: { color: #xe69614 } }
                                sw5 := mod.widgets.NdeSwatchBox { draw_bg +: { color: #x3cbe5a } }
                                sw6 := mod.widgets.NdeSwatchBox { draw_bg +: { color: #xeb4678 } }
                                sw7 := mod.widgets.NdeSwatchBox { draw_bg +: { color: #x64748b } }
                            }
                        }
                    }
                    app_rule := mod.widgets.NdeRule {}

                    // ---- Header ----
                    sect_hdr := View {
                        width: Fill height: Fit flow: Down spacing: 9.0
                        padding: Inset{left: 16.0, right: 16.0, top: 12.0, bottom: 14.0}
                        mod.widgets.NdeSection { text: "HEADER" draw_text +: { color: #x8a97a6 } }
                        hdr_show_field := View {
                            width: Fill height: Fit flow: Right align: Align{y: 0.5} spacing: 12.0
                            mod.widgets.NdeCtrl { width: 66.0 text: "Show" draw_text +: { color: #x6a7686 } }
                            tog_hdr_show := mod.widgets.NdeToggleView {}
                        }
                        hdr_style_field := View {
                            width: Fill height: Fit flow: Right align: Align{y: 0.5} spacing: 12.0
                            mod.widgets.NdeCtrl { width: 66.0 text: "Style" draw_text +: { color: #x6a7686 } }
                            seg_style := mod.widgets.NdeFrameBox {
                                width: Fit height: 34.0 flow: Right
                                draw_bg +: { color: #x00000000 }
                                seg_band := mod.widgets.NdeSegBox {
                                    width: Fit height: Fill flow: Right align: Align{x: 0.5, y: 0.5}
                                    padding: Inset{left: 13.0, right: 13.0, top: 0.0, bottom: 0.0}
                                    draw_bg +: { sel: 1.0 }
                                    seg_band_lbl := mod.widgets.NdeCtrl { text: "Plain" }
                                }
                                seg_fill := mod.widgets.NdeSegBox {
                                    width: Fit height: Fill flow: Right align: Align{x: 0.5, y: 0.5}
                                    padding: Inset{left: 13.0, right: 13.0, top: 0.0, bottom: 0.0}
                                    seg_fill_lbl := mod.widgets.NdeCtrl { text: "Fill" }
                                }
                            }
                        }
                        mod.widgets.NdeSection { text: "ALLOWED STEREOTYPES" draw_text +: { color: #x9aa6b4 font_size: 9 } }
                        tagfield := mod.widgets.NdeFrameBox {
                            width: Fill height: Fit flow: Right align: Align{y: 0.5} spacing: 4.0
                            padding: Inset{left: 8.0, right: 8.0, top: 6.0, bottom: 6.0}
                            draw_bg +: { color: #xffffff }
                            chip0 := mod.widgets.NdeTagChipBox {
                                width: Fit height: 34.0 flow: Right align: Align{x: 0.5, y: 0.5}
                                padding: Inset{left: 9.0, right: 9.0, top: 0.0, bottom: 0.0}
                                chip0_lbl := mod.widgets.NdeCtrl { text: "\u{ab}entity\u{bb}" draw_text +: { color: #xffffff } }
                            }
                            chip1 := mod.widgets.NdeTagChipBox {
                                width: Fit height: 34.0 flow: Right align: Align{x: 0.5, y: 0.5}
                                padding: Inset{left: 9.0, right: 9.0, top: 0.0, bottom: 0.0}
                                chip1_lbl := mod.widgets.NdeCtrl { text: "\u{ab}aggregate\u{bb}" draw_text +: { color: #xffffff } }
                            }
                            // PLACEHOLDER: adding stereotypes needs a hand-rolled text
                            // entry -- deferred; this is a display-only placeholder.
                            mod.widgets.NdeCtrl { width: Fill text: "Add stereotype\u{2026}" draw_text +: { color: #xa4b0bd } }
                        }
                        mod.widgets.NdeCtrl { text: "Shown in order. Empty will show all." draw_text +: { color: #x9aa6b4 } }
                        hdr_render_field := View {
                            width: Fill height: Fit flow: Right align: Align{y: 0.5} spacing: 12.0
                            mod.widgets.NdeCtrl { width: 66.0 text: "Render" draw_text +: { color: #x6a7686 } }
                            seg_render := mod.widgets.NdeFrameBox {
                                width: Fill height: 34.0 flow: Right
                                draw_bg +: { color: #x00000000 }
                                seg_r0 := mod.widgets.NdeSegBox { width: Fill height: Fill flow: Right align: Align{x: 0.5, y: 0.5} draw_bg +: { sel: 1.0 } seg_r0_lbl := mod.widgets.NdeCtrl { text: "All" } }
                                seg_r1 := mod.widgets.NdeSegBox { width: Fill height: Fill flow: Right align: Align{x: 0.5, y: 0.5} seg_r1_lbl := mod.widgets.NdeCtrl { text: "1" } }
                                seg_r2 := mod.widgets.NdeSegBox { width: Fill height: Fill flow: Right align: Align{x: 0.5, y: 0.5} seg_r2_lbl := mod.widgets.NdeCtrl { text: "2" } }
                                seg_r3 := mod.widgets.NdeSegBox { width: Fill height: Fill flow: Right align: Align{x: 0.5, y: 0.5} seg_r3_lbl := mod.widgets.NdeCtrl { text: "3" } }
                                seg_r4 := mod.widgets.NdeSegBox { width: Fill height: Fill flow: Right align: Align{x: 0.5, y: 0.5} seg_r4_lbl := mod.widgets.NdeCtrl { text: "4" } }
                                seg_r5 := mod.widgets.NdeSegBox { width: Fill height: Fill flow: Right align: Align{x: 0.5, y: 0.5} seg_r5_lbl := mod.widgets.NdeCtrl { text: "5" } }
                            }
                        }
                    }
                    hdr_rule := mod.widgets.NdeRule {}

                    // ---- Body ----
                    sect_body := View {
                        width: Fill height: Fit flow: Down spacing: 0.0
                        padding: Inset{left: 16.0, right: 16.0, top: 12.0, bottom: 14.0}
                        mod.widgets.NdeSection { text: "BODY \u{b7} DRAG TO REORDER" draw_text +: { color: #x8a97a6 } }
                        // PLACEHOLDER: drag-to-reorder is not wired (the grip is
                        // inert); order is static. Reordering would just permute the
                        // compartments, which the preview already mirrors.
                        item_at := View {
                            width: Fill height: Fit flow: Down spacing: 0.0
                            margin: Inset{left: 0.0, right: 0.0, top: 9.0, bottom: 0.0}
                            item_at_top := mod.widgets.NdeRule { draw_bg +: { a: 0.16 } }
                            at_crow := View {
                                width: Fill height: Fit flow: Right align: Align{y: 0.5} spacing: 9.0
                                padding: Inset{left: 6.0, right: 6.0, top: 6.0, bottom: 6.0}
                                grip_at := mod.widgets.NdeGripBox {}
                                at_clbl := mod.widgets.NdeCtrl { width: Fill text: "Attributes" draw_text +: { color: #x26313f } }
                                tog_at := mod.widgets.NdeToggleView {}
                            }
                            at_cols := View {
                                width: Fill height: Fit flow: Right spacing: 5.0
                                padding: Inset{left: 23.0, right: 0.0, top: 0.0, bottom: 8.0}
                                col_at_name := mod.widgets.NdeChipBox { width: Fit height: 28.0 flow: Right align: Align{x: 0.5, y: 0.5} padding: Inset{left: 9.0, right: 9.0, top: 0.0, bottom: 0.0} col_at_name_lbl := mod.widgets.NdeCtrl { text: "Name" draw_text +: { color: #x7b8797 font_size: 10 } } }
                                col_at_vis := mod.widgets.NdeChipBox { width: Fit height: 28.0 flow: Right align: Align{x: 0.5, y: 0.5} padding: Inset{left: 9.0, right: 9.0, top: 0.0, bottom: 0.0} draw_bg +: { on: 1.0 } col_at_vis_lbl := mod.widgets.NdeCtrl { text: "Visibility" draw_text +: { color: #x22303c font_size: 10 } } }
                                col_at_ty := mod.widgets.NdeChipBox { width: Fit height: 28.0 flow: Right align: Align{x: 0.5, y: 0.5} padding: Inset{left: 9.0, right: 9.0, top: 0.0, bottom: 0.0} draw_bg +: { on: 1.0 } col_at_ty_lbl := mod.widgets.NdeCtrl { text: "Type" draw_text +: { color: #x22303c font_size: 10 } } }
                                col_at_card := mod.widgets.NdeChipBox { width: Fit height: 28.0 flow: Right align: Align{x: 0.5, y: 0.5} padding: Inset{left: 9.0, right: 9.0, top: 0.0, bottom: 0.0} col_at_card_lbl := mod.widgets.NdeCtrl { text: "Cardinality" draw_text +: { color: #x8a97a6 font_size: 10 } } }
                            }
                        }
                        item_op := View {
                            width: Fill height: Fit flow: Down spacing: 0.0
                            margin: Inset{left: 0.0, right: 0.0, top: -1.0, bottom: 0.0}
                            item_op_top := mod.widgets.NdeRule { draw_bg +: { a: 0.16 } }
                            op_crow := View {
                                width: Fill height: Fit flow: Right align: Align{y: 0.5} spacing: 9.0
                                padding: Inset{left: 6.0, right: 6.0, top: 6.0, bottom: 6.0}
                                grip_op := mod.widgets.NdeGripBox {}
                                op_clbl := mod.widgets.NdeCtrl { width: Fill text: "Operations" draw_text +: { color: #xaab4c1 } }
                                tog_op := mod.widgets.NdeToggleView {}
                            }
                            op_cols := View {
                                width: Fill height: Fit flow: Right spacing: 5.0
                                padding: Inset{left: 23.0, right: 0.0, top: 0.0, bottom: 8.0}
                                col_op_name := mod.widgets.NdeChipBox { width: Fit height: 28.0 flow: Right align: Align{x: 0.5, y: 0.5} padding: Inset{left: 9.0, right: 9.0, top: 0.0, bottom: 0.0} draw_bg +: { enabled: 0.0 } col_op_name_lbl := mod.widgets.NdeCtrl { text: "Name" draw_text +: { color: #xaab4c1 font_size: 10 } } }
                                col_op_vis := mod.widgets.NdeChipBox { width: Fit height: 28.0 flow: Right align: Align{x: 0.5, y: 0.5} padding: Inset{left: 9.0, right: 9.0, top: 0.0, bottom: 0.0} draw_bg +: { on: 1.0 enabled: 0.0 } col_op_vis_lbl := mod.widgets.NdeCtrl { text: "Visibility" draw_text +: { color: #xaab4c1 font_size: 10 } } }
                                col_op_par := mod.widgets.NdeChipBox { width: Fit height: 28.0 flow: Right align: Align{x: 0.5, y: 0.5} padding: Inset{left: 9.0, right: 9.0, top: 0.0, bottom: 0.0} draw_bg +: { on: 1.0 enabled: 0.0 } col_op_par_lbl := mod.widgets.NdeCtrl { text: "Params" draw_text +: { color: #xaab4c1 font_size: 10 } } }
                                col_op_ret := mod.widgets.NdeChipBox { width: Fit height: 28.0 flow: Right align: Align{x: 0.5, y: 0.5} padding: Inset{left: 9.0, right: 9.0, top: 0.0, bottom: 0.0} draw_bg +: { on: 1.0 enabled: 0.0 } col_op_ret_lbl := mod.widgets.NdeCtrl { text: "Return" draw_text +: { color: #xaab4c1 font_size: 10 } } }
                            }
                        }
                        item_bottom := mod.widgets.NdeRule { draw_bg +: { a: 0.16 } }
                    }
                    body_rule := mod.widgets.NdeRule {}

                    // ---- Ports ----
                    sect_ports := View {
                        width: Fill height: Fit flow: Down spacing: 9.0
                        padding: Inset{left: 16.0, right: 16.0, top: 12.0, bottom: 14.0}
                        mod.widgets.NdeSection { text: "PORTS" draw_text +: { color: #x8a97a6 } }
                        ports_field := View {
                            width: Fill height: Fit flow: Right align: Align{y: 0.5} spacing: 12.0
                            mod.widgets.NdeCtrl { width: 66.0 text: "Show" draw_text +: { color: #x6a7686 } }
                            tog_ports := mod.widgets.NdeToggleView {}
                        }
                    }
                }
            }
        }
    }
}

/// RGB hex (no alpha) -> opaque `Vec4`, matching how the DSL decodes `#xrrggbb`.
fn rgb(hex: u32) -> Vec4 {
    Vec4 {
        x: ((hex >> 16) & 0xff) as f32 / 255.0,
        y: ((hex >> 8) & 0xff) as f32 / 255.0,
        z: (hex & 0xff) as f32 / 255.0,
        w: 1.0,
    }
}

/// The 8 accent swatches, in the mock's order (the Atlas `bucket_*` hexes). Kept
/// as Rust consts so `handle_event` can recolour every accent-material surface
/// per draw without reading the live theme back out.
const ACCENTS: [u32; 8] = [
    0x1496dc, 0x00b4d2, 0x14bea0, 0x5a6ef0, 0xe69614, 0x3cbe5a, 0xeb4678, 0x64748b,
];

/// An interactive region, resolved on `FingerUp` against its laid-out child rect.
#[derive(Clone, Copy, PartialEq)]
enum Region {
    /// Accent swatch `n` (0..8).
    Swatch(usize),
    /// Header "Show" toggle.
    HeaderShow,
    /// Ports "Show" toggle.
    PortsShow,
    /// A body compartment's on/off toggle: 0 = Attributes, 1 = Operations.
    CompToggle(usize),
    /// Header Style segmented: `true` = Fill, `false` = Band.
    HeaderStyle(bool),
    /// Render cap segmented: 0 = All, else 1..=5.
    Render(usize),
    /// A compartment column chip: (comp 0/1, column 1..3). Column 0 (Name) is
    /// locked and records no region.
    Column(usize, usize),
    /// Remove stereotype chip `n` (0/1).
    ChipRemove(usize),
}

/// The full live-preview model. Every control mutates one of these; the preview
/// re-reads them each draw. Seeded to the mock's defaults.
#[derive(Clone)]
struct PreviewState {
    header_show: bool,
    /// false = Band (hairline under header), true = Fill (accent-washed band).
    header_fill: bool,
    /// Allowed stereotypes; `None` marks a removed chip (the fixed 2-chip pool).
    stereotypes: [Option<String>; 2],
    /// Render cap: 0 = All, else 1..=5.
    render_cap: usize,
    /// Compartment on/off: [Attributes, Operations].
    comp_on: [bool; 2],
    /// Attribute columns [visibility, type, cardinality] (Name locked on).
    at_cols: [bool; 3],
    /// Operation columns [visibility, params, return] (Name locked on).
    op_cols: [bool; 3],
    ports_show: bool,
}

impl PreviewState {
    fn seed() -> Self {
        Self {
            header_show: true,
            header_fill: false,
            stereotypes: [Some("entity".into()), Some("aggregate".into())],
            render_cap: 0,
            comp_on: [true, false],
            at_cols: [true, true, false],
            op_cols: [true, true, true],
            ports_show: false,
        }
    }

    /// Allowed stereotypes still present, capped by Render (0 = All), in order.
    fn shown_stereotypes(&self) -> Vec<String> {
        let live: Vec<String> = self.stereotypes.iter().flatten().cloned().collect();
        let cap = if self.render_cap == 0 { live.len() } else { self.render_cap };
        live.into_iter().take(cap).collect()
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct NodeDesignEditor {
    /// Container: owns the whole DSL tree (pane / preview / controls) and the
    /// event area. Interaction is routed off its laid-out child areas.
    #[deref]
    view: View,

    /// Port-nub material overlay (drawn abs off the node's measured area rect,
    /// like the mock's absolute `.ports i` dots straddling the border).
    #[redraw]
    #[live]
    draw_nub: DrawColor,

    // ---- preview-card pens (the reusable node-render path, drawn abs) ----
    /// Glass frame + accent border of the preview node card.
    #[redraw]
    #[live]
    draw_frame: DrawColor,
    /// Flat fill for the header accent wash + compartment dividers.
    #[redraw]
    #[live]
    draw_rule: DrawColor,
    #[redraw]
    #[live]
    draw_mono_bold: DrawText,
    #[redraw]
    #[live]
    draw_mono_dim: DrawText,
    #[redraw]
    #[live]
    draw_mono_accent: DrawText,
    #[redraw]
    #[live]
    draw_mono_amber: DrawText,

    /// Selected accent (index into `ACCENTS`).
    #[rust]
    accent_idx: usize,
    /// Interactive hit rects captured during `draw_walk` from real child areas,
    /// matched in `handle_event` on `FingerUp`. Rebuilt every draw.
    #[rust]
    regions: Vec<(Region, Rect)>,

    #[rust(PreviewState::seed())]
    state: PreviewState,
}

impl NodeDesignEditor {
    /// Current accent colour (opaque) -- the *node's* designed accent, driven by
    /// the swatch. Only the preview card reads this.
    fn accent(&self) -> Vec4 {
        rgb(ACCENTS[self.accent_idx])
    }

    /// The editor chrome's own fixed accent (Atlas blue). Chrome does NOT follow
    /// the swatch -- picking a colour restyles the node, not the UI.
    fn ui_accent(&self) -> Vec4 {
        rgb(ACCENTS[0])
    }

    fn apply_region(&mut self, region: Region) {
        match region {
            Region::Swatch(i) => self.accent_idx = i,
            Region::HeaderShow => self.state.header_show = !self.state.header_show,
            Region::PortsShow => self.state.ports_show = !self.state.ports_show,
            Region::CompToggle(i) => self.state.comp_on[i] = !self.state.comp_on[i],
            Region::HeaderStyle(fill) => self.state.header_fill = fill,
            Region::Render(n) => self.state.render_cap = n,
            Region::Column(ci, col) => {
                let idx = col - 1;
                let cols = if ci == 0 { &mut self.state.at_cols } else { &mut self.state.op_cols };
                cols[idx] = !cols[idx];
            }
            Region::ChipRemove(n) => self.state.stereotypes[n] = None,
        }
    }
}

impl Widget for NodeDesignEditor {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        if let Hit::FingerUp(fe) = event.hits(cx, self.view.area()) {
            if !fe.is_primary_hit() {
                return;
            }
            let Some(region) = self
                .regions
                .iter()
                .find(|(_, r)| r.contains(fe.abs))
                .map(|(reg, _)| *reg)
            else {
                return;
            };
            self.apply_region(region);
            self.view.redraw(cx);
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // Push all state into the tree (visibility / text / colours / accent
        // uniforms) BEFORE the Turtle lays it out.
        self.sync(cx);
        while self.view.draw_walk(cx, scope, walk).step().is_some() {}
        // With the tree laid out, harvest interactive child rects for routing and
        // paint the preview node card (frame + wash + dividers + text + nubs) abs
        // off the stagecell area, from the shared `card::measure` geometry.
        self.capture_regions(cx);
        self.draw_card(cx);
        DrawStep::done()
    }
}

impl NodeDesignEditor {
    /// Push `accent` into a child View's `draw_bg` (the shared uniform name every
    /// accent-material shader reads).
    fn tint(&self, cx: &mut Cx2d, path: &[LiveId], accent: &[f32; 4]) {
        if let Some(mut v) = self.view.view(cx, path).borrow_mut() {
            v.draw_bg.set_uniform(cx, live_id!(accent), accent);
        }
    }

    /// Push a scalar uniform into a child View's `draw_bg`.
    fn set_bg(&self, cx: &mut Cx2d, path: &[LiveId], name: LiveId, v: f32) {
        if let Some(mut view) = self.view.view(cx, path).borrow_mut() {
            view.draw_bg.set_uniform(cx, name, &[v]);
        }
    }

    fn set_vis(&self, cx: &mut Cx, path: &[LiveId], on: bool) {
        self.view.view(cx, path).set_visible(cx, on);
    }

    fn set_label(&self, cx: &mut Cx, path: &[LiveId], color: Vec4) {
        if let Some(mut l) = self.view.label(cx, path).borrow_mut() {
            l.draw_text.color = color;
        }
    }

    /// Fold the whole `state` into the tree ahead of layout. (The preview node
    /// itself is not in the tree -- it is drawn abs in `draw_card` from the
    /// shared `card::measure` geometry; this only syncs the chrome.)
    fn sync(&mut self, cx: &mut Cx2d) {
        // The chrome carries a FIXED accent -- only the preview node (drawn in
        // `draw_card`) follows the selected swatch.
        let ui = self.ui_accent();
        let ui_au = [ui.x, ui.y, ui.z, 1.0];
        let st = self.state.clone();

        // Accent-material surfaces: push the fixed UI accent to every one.
        for path in ACCENT_VIEWS {
            self.tint(cx, path, &ui_au);
        }

        // -- identity header --
        self.set_label(cx, ids!(pane.phead.meta.phead_stereo), ui);

        // -- controls: swatch selection rings --
        for (i, path) in SWATCH_VIEWS.iter().enumerate() {
            self.set_bg(cx, path, live_id!(sel), if i == self.accent_idx { 1.0 } else { 0.0 });
        }

        // -- controls: toggles --
        self.set_bg(cx, ids!(pane.editor.controls.sect_hdr.hdr_show_field.tog_hdr_show),
            live_id!(on), if st.header_show { 1.0 } else { 0.0 });
        self.set_bg(cx, ids!(pane.editor.controls.sect_body.item_at.at_crow.tog_at),
            live_id!(on), if st.comp_on[0] { 1.0 } else { 0.0 });
        self.set_bg(cx, ids!(pane.editor.controls.sect_body.item_op.op_crow.tog_op),
            live_id!(on), if st.comp_on[1] { 1.0 } else { 0.0 });
        self.set_bg(cx, ids!(pane.editor.controls.sect_ports.ports_field.tog_ports),
            live_id!(on), if st.ports_show { 1.0 } else { 0.0 });

        // -- controls: Style segmented --
        self.sync_seg_cell(cx, ids!(pane.editor.controls.sect_hdr.hdr_style_field.seg_style.seg_band),
            ids!(pane.editor.controls.sect_hdr.hdr_style_field.seg_style.seg_band.seg_band_lbl), !st.header_fill);
        self.sync_seg_cell(cx, ids!(pane.editor.controls.sect_hdr.hdr_style_field.seg_style.seg_fill),
            ids!(pane.editor.controls.sect_hdr.hdr_style_field.seg_style.seg_fill.seg_fill_lbl), st.header_fill);

        // -- controls: Render segmented --
        for (i, (cell, lbl)) in RENDER_CELLS.iter().enumerate() {
            self.sync_seg_cell(cx, cell, lbl, i == st.render_cap);
        }

        // -- controls: compartment labels + grips + col chips --
        self.sync_body_controls(cx, &st);

        // -- controls: stereotype chips (hide removed) --
        self.set_vis(cx, ids!(pane.editor.controls.sect_hdr.tagfield.chip0), st.stereotypes[0].is_some());
        self.set_vis(cx, ids!(pane.editor.controls.sect_hdr.tagfield.chip1), st.stereotypes[1].is_some());
    }

    /// White text on the selected cell, dim on the rest.
    fn sync_seg_cell(&self, cx: &mut Cx2d, cell: &[LiveId], lbl: &[LiveId], on: bool) {
        self.set_bg(cx, cell, live_id!(sel), if on { 1.0 } else { 0.0 });
        self.set_label(cx, lbl, if on { rgb(0xffffff) } else { rgb(0x7b8797) });
    }

    /// Compartment row labels, grips, and column-chip on/enabled states.
    fn sync_body_controls(&self, cx: &mut Cx2d, st: &PreviewState) {
        let accent = self.ui_accent();
        let grip_at = if st.comp_on[0] { accent } else { rgb(0xb3bdca) };
        let grip_op = if st.comp_on[1] { accent } else { rgb(0xb3bdca) };
        self.set_bg_col(cx, ids!(pane.editor.controls.sect_body.item_at.at_crow.grip_at), grip_at);
        self.set_bg_col(cx, ids!(pane.editor.controls.sect_body.item_op.op_crow.grip_op), grip_op);
        self.set_label(cx, ids!(pane.editor.controls.sect_body.item_at.at_crow.at_clbl),
            if st.comp_on[0] { rgb(0x26313f) } else { rgb(0xaab4c1) });
        self.set_label(cx, ids!(pane.editor.controls.sect_body.item_op.op_crow.op_clbl),
            if st.comp_on[1] { rgb(0x26313f) } else { rgb(0xaab4c1) });

        // Attribute column chips (index 1..3 -> at_cols[0..2]).
        self.sync_chip(cx, AT_CHIPS[0].0, AT_CHIPS[0].1, st.at_cols[0], st.comp_on[0]);
        self.sync_chip(cx, AT_CHIPS[1].0, AT_CHIPS[1].1, st.at_cols[1], st.comp_on[0]);
        self.sync_chip(cx, AT_CHIPS[2].0, AT_CHIPS[2].1, st.at_cols[2], st.comp_on[0]);
        // Operation column chips.
        self.sync_chip(cx, OP_CHIPS[0].0, OP_CHIPS[0].1, st.op_cols[0], st.comp_on[1]);
        self.sync_chip(cx, OP_CHIPS[1].0, OP_CHIPS[1].1, st.op_cols[1], st.comp_on[1]);
        self.sync_chip(cx, OP_CHIPS[2].0, OP_CHIPS[2].1, st.op_cols[2], st.comp_on[1]);
    }

    /// A column chip's fill (`on`) / dim (`enabled`) uniforms + text colour.
    fn sync_chip(&self, cx: &mut Cx2d, cell: &[LiveId], lbl: &[LiveId], on: bool, enabled: bool) {
        self.set_bg(cx, cell, live_id!(on), if on { 1.0 } else { 0.0 });
        self.set_bg(cx, cell, live_id!(enabled), if enabled { 1.0 } else { 0.0 });
        let color = if !enabled {
            rgb(0xaab4c1)
        } else if on {
            rgb(0x22303c)
        } else {
            rgb(0x8a97a6)
        };
        self.set_label(cx, lbl, color);
    }

    /// Push a colour into a glyph shader's `col` uniform (grip).
    fn set_bg_col(&self, cx: &mut Cx2d, path: &[LiveId], c: Vec4) {
        if let Some(mut v) = self.view.view(cx, path).borrow_mut() {
            v.draw_bg.set_uniform(cx, live_id!(col), &[c.x, c.y, c.z, c.w]);
        }
    }

    /// Read every interactive child's laid-out area rect for `FingerUp` routing.
    fn capture_regions(&mut self, cx: &mut Cx2d) {
        self.regions.clear();
        // Swatches.
        for (i, path) in SWATCH_VIEWS.iter().enumerate() {
            self.push_region(cx, Region::Swatch(i), path);
        }
        // Toggles.
        self.push_region(cx, Region::HeaderShow, ids!(pane.editor.controls.sect_hdr.hdr_show_field.tog_hdr_show));
        self.push_region(cx, Region::PortsShow, ids!(pane.editor.controls.sect_ports.ports_field.tog_ports));
        // Style segmented (withheld when the header is off, matching the mock's
        // inert faded row).
        if self.state.header_show {
            self.push_region(cx, Region::HeaderStyle(false), ids!(pane.editor.controls.sect_hdr.hdr_style_field.seg_style.seg_band));
            self.push_region(cx, Region::HeaderStyle(true), ids!(pane.editor.controls.sect_hdr.hdr_style_field.seg_style.seg_fill));
            for (i, (cell, _)) in RENDER_CELLS.iter().enumerate() {
                self.push_region(cx, Region::Render(i), cell);
            }
        }
        // Stereotype chips: clicking removes (the mock's hover-✕, simplified).
        if self.state.stereotypes[0].is_some() {
            self.push_region(cx, Region::ChipRemove(0), ids!(pane.editor.controls.sect_hdr.tagfield.chip0));
        }
        if self.state.stereotypes[1].is_some() {
            self.push_region(cx, Region::ChipRemove(1), ids!(pane.editor.controls.sect_hdr.tagfield.chip1));
        }
        // Body compartment toggles.
        self.push_region(cx, Region::CompToggle(0), ids!(pane.editor.controls.sect_body.item_at.at_crow.tog_at));
        self.push_region(cx, Region::CompToggle(1), ids!(pane.editor.controls.sect_body.item_op.op_crow.tog_op));
        // Column chips (locked Name chips are not interactive).
        if self.state.comp_on[0] {
            self.push_region(cx, Region::Column(0, 1), AT_CHIPS[0].0);
            self.push_region(cx, Region::Column(0, 2), AT_CHIPS[1].0);
            self.push_region(cx, Region::Column(0, 3), AT_CHIPS[2].0);
        }
        if self.state.comp_on[1] {
            self.push_region(cx, Region::Column(1, 1), OP_CHIPS[0].0);
            self.push_region(cx, Region::Column(1, 2), OP_CHIPS[1].0);
            self.push_region(cx, Region::Column(1, 3), OP_CHIPS[2].0);
        }
    }

    fn push_region(&mut self, cx: &mut Cx2d, region: Region, path: &[LiveId]) {
        let r = self.view.view(cx, path).area().rect(cx);
        if r.size.x > 0.0 && r.size.y > 0.0 {
            self.regions.push((region, r));
        }
    }

    /// Build the preview `SceneNode` from `PreviewState`. Column toggles map to
    /// field-clearing (`class_shape` omits empty parts); the compartment toggles
    /// map to whether the attribute/operation rows are present at all. This is
    /// the projection the shared node-render path consumes.
    fn preview_node(&self) -> crate::scene::SceneNode {
        use crate::inspector::{AttrRow, OpRow};
        use crate::scene::{HeaderStyle, SceneNode};
        let st = &self.state;

        let header = if !st.header_show {
            HeaderStyle::Hidden
        } else if st.header_fill {
            HeaderStyle::Fill
        } else {
            HeaderStyle::Plain
        };

        // Allowed stereotypes capped by Render drive the «guillemet» eyebrow;
        // empty falls back to «entity» like the mock.
        let shown = st.shown_stereotypes();
        let stereotypes = if shown.is_empty() {
            vec!["entity".to_string()]
        } else {
            shown
        };

        let attributes = if st.comp_on[0] {
            let mk = |vis: &str, name: &str, ty: &str, card: &str| AttrRow {
                name: name.to_string(),
                ty: if st.at_cols[1] { ty.to_string() } else { String::new() },
                multiplicity: if st.at_cols[2] { card.to_string() } else { String::new() },
                visibility: if st.at_cols[0] { vis.to_string() } else { String::new() },
            };
            vec![
                mk("+", "id", "UUID", "1"),
                mk("+", "total", "Money", "1"),
                mk("-", "items", "Line", "1..*"),
            ]
        } else {
            Vec::new()
        };

        let operations = if st.comp_on[1] {
            let mk = |vis: &str, name: &str, params: &str, ret: &str| OpRow {
                name: name.to_string(),
                params: if st.op_cols[1] {
                    Some(params.to_string())
                } else {
                    None
                },
                ret: if st.op_cols[2] { ret.to_string() } else { String::new() },
                visibility: if st.op_cols[0] { vis.to_string() } else { String::new() },
            };
            vec![
                mk("+", "place", "pay", "void"),
                mk("+", "cancel", "", "void"),
            ]
        } else {
            Vec::new()
        };

        SceneNode {
            key: "preview".to_string(),
            title: "Order".to_string(),
            element_type: waml::model::ElementType::Uml(waml::model::UmlMetaclass::Class),
            stereotypes,
            attributes,
            operations,
            header,
            ports: st.ports_show,
            rect: waml::solve::Rect {
                x: 0.0,
                y: 0.0,
                w: 0.0,
                h: 0.0,
            },
            emphasized: true,
            collapsed: false,
        }
    }

    /// Draw the preview node card abs, centred in the stagecell ground, from the
    /// shared `card::measure` geometry (the same path the real canvas walks):
    /// glass frame, header wash (Fill), compartment dividers, mono text, and the
    /// port nubs straddling the border. The accent pen is re-coloured live so a
    /// swatch pick recolours the whole card.
    fn draw_card(&mut self, cx: &mut Cx2d) {
        use crate::card::{self, Token, Weight};
        use crate::scene::HeaderStyle;

        let stage = self
            .view
            .view(cx, ids!(pane.editor.stagecell))
            .area()
            .rect(cx);
        if stage.size.x <= 0.0 {
            return;
        }
        let node = self.preview_node();
        let placed = card::measure(&card::class_shape(&node, &card::mono_sheet()));
        let (cw, ch) = placed.size;
        let ox = stage.pos.x + (stage.size.x - cw) * 0.5;
        let oy = stage.pos.y + (stage.size.y - ch) * 0.5;
        let accent = self.accent();
        let au = [accent.x, accent.y, accent.z, 1.0];

        // Glass frame + accent border.
        self.draw_frame.set_uniform(cx, live_id!(accent), &au);
        self.draw_frame.color = rgb(0xffffff);
        self.draw_frame.draw_abs(
            cx,
            Rect {
                pos: dvec2(ox, oy),
                size: dvec2(cw, ch),
            },
        );

        // Header accent wash (Fill only).
        if node.header == HeaderStyle::Fill {
            if let Some(h) = placed.header() {
                let bottom = h.y + h.h + h.y; // symmetric inset around the header
                self.draw_rule.color = Vec4 {
                    x: accent.x,
                    y: accent.y,
                    z: accent.z,
                    w: 0.12,
                };
                self.draw_rule.draw_abs(
                    cx,
                    Rect {
                        pos: dvec2(ox, oy),
                        size: dvec2(cw, bottom),
                    },
                );
            }
        }

        // Inter-compartment dividers.
        for dy in placed.compartment_dividers() {
            self.draw_rule.color = Vec4 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 0.10,
            };
            self.draw_rule.draw_abs(
                cx,
                Rect {
                    pos: dvec2(ox, oy + dy - 0.5),
                    size: dvec2(cw, 1.0),
                },
            );
        }

        // Mono text leaves, coloured by (weight, Atlas token).
        self.draw_mono_accent.color = accent;
        for pt in &placed.texts {
            let pos = dvec2(ox + pt.x, oy + pt.y);
            let size = pt.style.size_pt as f32;
            match (pt.style.weight, pt.style.color) {
                (Weight::Bold, _) => {
                    self.draw_mono_bold.text_style.font_size = size;
                    self.draw_mono_bold.draw_abs(cx, pos, &pt.text);
                }
                (Weight::Regular, Token::Accent) => {
                    self.draw_mono_accent.text_style.font_size = size;
                    self.draw_mono_accent.draw_abs(cx, pos, &pt.text);
                }
                (Weight::Regular, Token::Amber) => {
                    self.draw_mono_amber.text_style.font_size = size;
                    self.draw_mono_amber.draw_abs(cx, pos, &pt.text);
                }
                (Weight::Regular, _) => {
                    self.draw_mono_dim.text_style.font_size = size;
                    self.draw_mono_dim.draw_abs(cx, pos, &pt.text);
                }
            }
        }

        // Port nubs straddling the card border (two left, one right).
        if node.ports {
            self.draw_nub.set_uniform(cx, live_id!(accent), &au);
            self.draw_nub.color = rgb(0xffffff);
            let sz = 8.0;
            let nubs = [
                dvec2(ox - 4.0, oy + ch * 0.44),
                dvec2(ox - 4.0, oy + ch * 0.66),
                dvec2(ox + cw - 4.0, oy + ch * 0.52),
            ];
            for p in nubs {
                self.draw_nub.draw_abs(cx, Rect { pos: p, size: dvec2(sz, sz) });
            }
        }
    }
}

// ---- id tables (kept next to the tree so the paths stay in sync) ----

const SWATCH_VIEWS: [&[LiveId]; 8] = [
    ids!(pane.editor.controls.sect_app.app_field.swatches.sw0),
    ids!(pane.editor.controls.sect_app.app_field.swatches.sw1),
    ids!(pane.editor.controls.sect_app.app_field.swatches.sw2),
    ids!(pane.editor.controls.sect_app.app_field.swatches.sw3),
    ids!(pane.editor.controls.sect_app.app_field.swatches.sw4),
    ids!(pane.editor.controls.sect_app.app_field.swatches.sw5),
    ids!(pane.editor.controls.sect_app.app_field.swatches.sw6),
    ids!(pane.editor.controls.sect_app.app_field.swatches.sw7),
];

/// Render-cap segmented cells + their labels (index 0 = All, 1..5).
const RENDER_CELLS: [(&[LiveId], &[LiveId]); 6] = [
    (ids!(pane.editor.controls.sect_hdr.hdr_render_field.seg_render.seg_r0), ids!(pane.editor.controls.sect_hdr.hdr_render_field.seg_render.seg_r0.seg_r0_lbl)),
    (ids!(pane.editor.controls.sect_hdr.hdr_render_field.seg_render.seg_r1), ids!(pane.editor.controls.sect_hdr.hdr_render_field.seg_render.seg_r1.seg_r1_lbl)),
    (ids!(pane.editor.controls.sect_hdr.hdr_render_field.seg_render.seg_r2), ids!(pane.editor.controls.sect_hdr.hdr_render_field.seg_render.seg_r2.seg_r2_lbl)),
    (ids!(pane.editor.controls.sect_hdr.hdr_render_field.seg_render.seg_r3), ids!(pane.editor.controls.sect_hdr.hdr_render_field.seg_render.seg_r3.seg_r3_lbl)),
    (ids!(pane.editor.controls.sect_hdr.hdr_render_field.seg_render.seg_r4), ids!(pane.editor.controls.sect_hdr.hdr_render_field.seg_render.seg_r4.seg_r4_lbl)),
    (ids!(pane.editor.controls.sect_hdr.hdr_render_field.seg_render.seg_r5), ids!(pane.editor.controls.sect_hdr.hdr_render_field.seg_render.seg_r5.seg_r5_lbl)),
];

/// Attribute column chips (cell, label) for cols 1..3 (Visibility, Type, Cardinality).
const AT_CHIPS: [(&[LiveId], &[LiveId]); 3] = [
    (ids!(pane.editor.controls.sect_body.item_at.at_cols.col_at_vis), ids!(pane.editor.controls.sect_body.item_at.at_cols.col_at_vis.col_at_vis_lbl)),
    (ids!(pane.editor.controls.sect_body.item_at.at_cols.col_at_ty), ids!(pane.editor.controls.sect_body.item_at.at_cols.col_at_ty.col_at_ty_lbl)),
    (ids!(pane.editor.controls.sect_body.item_at.at_cols.col_at_card), ids!(pane.editor.controls.sect_body.item_at.at_cols.col_at_card.col_at_card_lbl)),
];
/// Operation column chips (Visibility, Params, Return).
const OP_CHIPS: [(&[LiveId], &[LiveId]); 3] = [
    (ids!(pane.editor.controls.sect_body.item_op.op_cols.col_op_vis), ids!(pane.editor.controls.sect_body.item_op.op_cols.col_op_vis.col_op_vis_lbl)),
    (ids!(pane.editor.controls.sect_body.item_op.op_cols.col_op_par), ids!(pane.editor.controls.sect_body.item_op.op_cols.col_op_par.col_op_par_lbl)),
    (ids!(pane.editor.controls.sect_body.item_op.op_cols.col_op_ret), ids!(pane.editor.controls.sect_body.item_op.op_cols.col_op_ret.col_op_ret_lbl)),
];

/// Every accent-material surface: the accent uniform is pushed to each per draw so
/// picking a swatch recolours the whole pane + preview + controls.
const ACCENT_VIEWS: [&[LiveId]; 43] = [
    ids!(pane),
    ids!(pane.phead.ico),
    ids!(pane.phead_rule),
    ids!(pane.editor.body_divider),
    ids!(pane.editor.controls.sect_app.app_field.swatches.sw0),
    ids!(pane.editor.controls.sect_app.app_field.swatches.sw1),
    ids!(pane.editor.controls.sect_app.app_field.swatches.sw2),
    ids!(pane.editor.controls.sect_app.app_field.swatches.sw3),
    ids!(pane.editor.controls.sect_app.app_field.swatches.sw4),
    ids!(pane.editor.controls.sect_app.app_field.swatches.sw5),
    ids!(pane.editor.controls.sect_app.app_field.swatches.sw6),
    ids!(pane.editor.controls.sect_app.app_field.swatches.sw7),
    ids!(pane.editor.controls.app_rule),
    ids!(pane.editor.controls.sect_hdr.hdr_show_field.tog_hdr_show),
    ids!(pane.editor.controls.sect_hdr.hdr_style_field.seg_style),
    ids!(pane.editor.controls.sect_hdr.hdr_style_field.seg_style.seg_band),
    ids!(pane.editor.controls.sect_hdr.hdr_style_field.seg_style.seg_fill),
    ids!(pane.editor.controls.sect_hdr.tagfield),
    ids!(pane.editor.controls.sect_hdr.tagfield.chip0),
    ids!(pane.editor.controls.sect_hdr.tagfield.chip1),
    ids!(pane.editor.controls.sect_hdr.hdr_render_field.seg_render),
    ids!(pane.editor.controls.sect_hdr.hdr_render_field.seg_render.seg_r0),
    ids!(pane.editor.controls.sect_hdr.hdr_render_field.seg_render.seg_r1),
    ids!(pane.editor.controls.sect_hdr.hdr_render_field.seg_render.seg_r2),
    ids!(pane.editor.controls.sect_hdr.hdr_render_field.seg_render.seg_r3),
    ids!(pane.editor.controls.sect_hdr.hdr_render_field.seg_render.seg_r4),
    ids!(pane.editor.controls.sect_hdr.hdr_render_field.seg_render.seg_r5),
    ids!(pane.editor.controls.hdr_rule),
    ids!(pane.editor.controls.sect_body.item_at.item_at_top),
    ids!(pane.editor.controls.sect_body.item_at.at_crow.tog_at),
    ids!(pane.editor.controls.sect_body.item_at.at_cols.col_at_name),
    ids!(pane.editor.controls.sect_body.item_at.at_cols.col_at_vis),
    ids!(pane.editor.controls.sect_body.item_at.at_cols.col_at_ty),
    ids!(pane.editor.controls.sect_body.item_at.at_cols.col_at_card),
    ids!(pane.editor.controls.sect_body.item_op.item_op_top),
    ids!(pane.editor.controls.sect_body.item_op.op_crow.tog_op),
    ids!(pane.editor.controls.sect_body.item_op.op_cols.col_op_name),
    ids!(pane.editor.controls.sect_body.item_op.op_cols.col_op_vis),
    ids!(pane.editor.controls.sect_body.item_op.op_cols.col_op_par),
    ids!(pane.editor.controls.sect_body.item_op.op_cols.col_op_ret),
    ids!(pane.editor.controls.sect_body.item_bottom),
    ids!(pane.editor.controls.body_rule),
    ids!(pane.editor.controls.sect_ports.ports_field.tog_ports),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_defaults_match_mock() {
        let s = PreviewState::seed();
        assert!(s.header_show && !s.header_fill);
        assert_eq!(s.comp_on, [true, false]);
        assert_eq!(s.at_cols, [true, true, false]);
        assert_eq!(s.op_cols, [true, true, true]);
    }

    #[test]
    fn render_cap_caps_shown_stereotypes() {
        let mut s = PreviewState::seed();
        assert_eq!(s.shown_stereotypes(), vec!["entity", "aggregate"]);
        s.render_cap = 1;
        assert_eq!(s.shown_stereotypes(), vec!["entity"]);
        s.render_cap = 0;
        s.stereotypes[0] = None;
        assert_eq!(s.shown_stereotypes(), vec!["aggregate"]);
    }
}
