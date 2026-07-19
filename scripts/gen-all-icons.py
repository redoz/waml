#!/usr/bin/env python3
"""Regenerate the Lucide-sourced glyphs of crates/waml-editor/src/icons.rs.

The custom UML glyphs (IconClass, IconInterface, IconEnum, IconDataType,
IconDiagram, IconFlow, IconSequence, IconNote -- no svg source) are hand-authored
and left untouched. Everything after IconNote is Lucide-sourced and regenerated
from the svg set at the faithful 1:1 viewBox fit (gen-icon.py A=1/24, B=0). One
exception: IconPackage (which lives inside the custom block) is re-bodied from
box.svg -- the box glyph doubles as the package icon.

Anchors are the stable custom-UML boundary (the `note` field) and Icon names, so
the script is idempotent: re-run any time (e.g. after dropping svgs from the dir
or editing EXISTING) against an already-generated icons.rs.

    python scripts/gen-all-icons.py
"""
import importlib.util
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ICONS_DIR = os.path.join(ROOT, "crates", "waml-editor", "resources", "icons")
ICONS_RS = os.path.join(ROOT, "crates", "waml-editor", "src", "icons.rs")

_spec = importlib.util.spec_from_file_location(
    "genicon", os.path.join(os.path.dirname(os.path.abspath(__file__)), "gen-icon.py"))
gi = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(gi)

RUST_KEYWORDS = {
    "as", "box", "break", "const", "continue", "crate", "dyn", "else", "enum",
    "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match",
    "mod", "move", "mut", "pub", "ref", "return", "self", "static", "struct",
    "super", "trait", "true", "type", "unsafe", "use", "where", "while", "async",
    "await", "abstract", "become", "do", "final", "gen", "macro", "override",
    "priv", "typeof", "unsized", "virtual", "yield", "try",
}


def body_lines(svg_stem):
    """Return the inner pixel-fn body lines for one svg (let w .. return)."""
    svg = open(os.path.join(ICONS_DIR, svg_stem + ".svg"), encoding="utf-8").read()
    elems = re.findall(r'<(path|circle|ellipse|rect|line|polyline|polygon)\b([^>]*)>', svg)
    ds = [d for name, tag in elems if (d := gi.element_d(name, tag))]
    if not ds:
        sys.exit("no drawable elements in " + svg_stem)
    lines = ["            let w = s * %.3f" % gi.STROKE_W,
             "            let sdf = Sdf2d.viewport(self.pos * self.rect_size)"]
    for d in ds:
        gi.emit_path(d, lines)
        lines.append("            sdf.stroke(self.color, w)")
    lines.append("            return sdf.result")
    return lines


def pascal(stem):
    return "Icon" + "".join(p.capitalize() for p in stem.split("-"))


def snake(stem):
    s = stem.replace("-", "_")
    return s + "_" if s in RUST_KEYWORDS else s


# (field, IconName, svg_stem, comment) -- the already-shipped Lucide glyphs keep
# their identity/order; every other svg is appended below alphabetically.
EXISTING = [
    ("message",        "IconMessage",       "message-square-text",  "Message: speech bubble with three text lines."),
    ("package_plus",   "IconPackagePlus",   "package-plus",         "Package plus: box with a + badge."),
    ("paintbrush",     "IconPaintbrush",    "paintbrush-vertical",  "Paintbrush (vertical): bristle head + handle."),
    ("pin",            "IconPin",           "pin",                  "Pin: map pin."),
    ("pin_off",        "IconPinOff",        "pin-off",              "Pin off: map pin with a slash."),
    ("share",          "IconShare",         "share",                "Share: node-link share glyph."),
    ("spline",         "IconSpline",        "spline",               "Spline: curve with control handles."),
    ("spline_pointer", "IconSplinePointer", "spline-pointer",       "Spline pointer: spline meeting a cursor."),
    ("square_minus",   "IconSquareMinus",   "square-minus",         "Square minus: rounded square with a minus."),
    ("square_plus",    "IconSquarePlus",    "square-plus",          "Square plus: rounded square with a plus."),
    ("trash",          "IconTrash",         "trash",                "Trash: waste bin."),
    ("list_collapse",  "IconListCollapse",  "list-chevrons-down-up","List collapse (down-up): rows + inward chevrons."),
    ("list_expand",    "IconListExpand",    "list-chevrons-up-down","List expand (up-down): rows + outward chevrons."),
    ("pencil",         "IconPencil",        "pencil",               "Pencil."),
    ("menu",           "IconMenu",          "menu",                 "Menu (hamburger): three rows."),
    ("moon",           "IconMoon",          "moon",                 "Moon: crescent."),
]

_MAPPED = {e[2] for e in EXISTING}
# waml = app logo (not a tree glyph); package = collides with the custom
# IconPackage field, which we instead re-body from box; box itself is consumed
# by that mapping (and `box` is a Rust keyword anyway).
PACKAGE_SVG = "box"
SKIP = {"waml", "package", "box"}


def build_manifest():
    manifest = list(EXISTING)
    skipped = []
    stems = sorted(os.path.splitext(f)[0] for f in os.listdir(ICONS_DIR) if f.endswith(".svg"))
    for stem in stems:
        if stem in _MAPPED:
            continue
        if stem in SKIP:
            skipped.append(stem)
            continue
        manifest.append((snake(stem), pascal(stem), stem,
                         "Faithful port of resources/icons/%s.svg via scripts/gen-icon.py." % stem))
    return manifest, skipped


def draw_block(field, icon, stem, comment):
    return ("    // %s\n"
            "    // Faithful port of resources/icons/%s.svg via scripts/gen-icon.py.\n"
            "    mod.draw.%s = mod.draw.DrawColor{\n"
            "        pixel: fn() {\n"
            "            let s = self.rect_size.x\n"
            "%s\n"
            "        }\n"
            "    }\n") % (comment, stem, icon, "\n".join(body_lines(stem)))


def replace_span(text, start_after, end_at, new_inner):
    """Replace text between the end of `start_after` and the start of `end_at`."""
    i = text.index(start_after) + len(start_after)
    j = text.index(end_at, i)
    return text[:i] + new_inner + text[j:]


def main():
    manifest, skipped = build_manifest()
    text = open(ICONS_RS, encoding="utf-8").read()

    # --- IconPackage: re-body in place from box.svg (stays in the custom block) ---
    pkg_at = text.index("mod.draw.IconPackage = mod.draw.DrawColor{")
    body_start = text.index("            let s = self.rect_size.x\n", pkg_at) \
        + len("            let s = self.rect_size.x\n")
    body_end = text.index("\n        }\n    }", body_start)
    text = text[:body_start] + "\n".join(body_lines(PACKAGE_SVG)) + text[body_end:]

    # --- Region A: draw blocks (everything after the IconNote block) ---
    note_at = text.index("mod.draw.IconNote = mod.draw.DrawColor{")
    first_lucide = text.index("\n\n    //", note_at) + 2  # start of first Lucide comment
    blocks = "\n".join(draw_block(*m) for m in manifest)
    text = text[:first_lucide] + blocks + "\n" + text[text.index("    mod.widgets.TreeIconsBase", first_lucide):]

    # --- Region B: init lines (after the custom `note:` init line) ---
    init = "\n" + "\n".join("        %s: mod.draw.%s{ color: atlas.accent }" % (f, ic)
                            for f, ic, _, _ in manifest)
    text = replace_span(text, "        note: mod.draw.IconNote{ color: atlas.accent }",
                        "\n    }\n}", init)

    # --- Region C: struct fields (after the custom `pub note` field) ---
    fields = "\n" + "\n".join("    #[live]\n    pub %s: DrawColor," % f for f, _, _, _ in manifest)
    text = replace_span(text, "    pub note: DrawColor,", "\n}\n\nimpl TreeIcons", fields)

    # --- Region D: labeled_mut entries + array count ---
    n = 9 + len(manifest)
    text = re.sub(r"-> \[\(&'static str, &mut DrawColor\); \d+\]",
                  "-> [(&'static str, &mut DrawColor); %d]" % n, text)
    labels = "\n" + "\n".join('            ("%s", &mut self.%s),' % (stem, f)
                              for f, _, stem, _ in manifest)
    text = replace_span(text, '            ("note", &mut self.note),', "\n        ]", labels)

    open(ICONS_RS, "w", encoding="utf-8").write(text)
    print("regenerated %d Lucide glyphs + IconPackage(from box); array=%d" % (len(manifest), n))
    if skipped:
        print("skipped: " + ", ".join(skipped))


if __name__ == "__main__":
    main()
