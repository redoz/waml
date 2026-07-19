#!/usr/bin/env python3
"""Generate an Icon<Name> body (crates/waml-editor/src/icons.rs) from a Lucide svg.

Generic successor to gen-pin-icon.py: instead of hand-transcribing one glyph's
path commands, this parses the svg's `d` attributes directly. Each <path> becomes
its own stroke run (Lucide draws with per-path strokes). The full path grammar is
supported: M/L/H/V/C/S/Q/T/A/Z, absolute + relative. Circular arcs (rx==ry, which
is all Lucide uses) map to sdf.arc_to via SVG endpoint->centre conversion; cubic
and quadratic beziers are flattened to line_to runs (Sdf2d has no bezier). Angles
are y-down screen space, 0 = +x, ccw+, matching the shader's atan2.

Fit: norm(c) = A*c + B, uniform + isotropic (A scales radii, angles pass through),
shared with the pin so every Lucide glyph lands at the same size/weight in-cell.
Revisit A/B/STROKE_W per-icon in the harness if the stroke clips the viewport.

Run:  python scripts/gen-icon.py resources/icons/<name>.svg   # prints DSL body
"""
import math
import re
import sys

# Shared Lucide fit (see gen-pin-icon.py): 24-space -> cell, stroke held inside
# the Sdf2d.viewport clip. Uniform + isotropic so arc angles pass through.
A = 0.042768
B = -0.013216
STROKE_W = 0.068  # half-width; Sdf2d.stroke treats w as half-width
CUBIC_STEPS = 12  # flatten resolution for C/S/Q/T


def nx(c):
    return A * c + B


def nr(r):
    return A * r


def arc_centre(x0, y0, x1, y1, r, large, sweep):
    """SVG circular-arc endpoint form -> (cx, cy, r, a0, a1). rx==ry assumed."""
    dx = (x0 - x1) / 2
    dy = (y0 - y1) / 2
    rc = (dx * dx + dy * dy) / (r * r)
    if rc > 1:
        r *= math.sqrt(rc)
    num = r * r * r * r - r * r * dy * dy - r * r * dx * dx
    den = r * r * dy * dy + r * r * dx * dx
    co = math.sqrt(max(0.0, num / den))
    if large == sweep:
        co = -co
    cx = co * (r * dy / r) + (x0 + x1) / 2
    cy = co * (-r * dx / r) + (y0 + y1) / 2

    def ang(ux, uy, vx, vy):
        dot = ux * vx + uy * vy
        ln = math.hypot(ux, uy) * math.hypot(vx, vy)
        a = math.acos(max(-1.0, min(1.0, dot / ln)))
        return -a if ux * vy - uy * vx < 0 else a

    a0 = ang(1, 0, (x0 - cx) / r, (y0 - cy) / r)
    dth = ang((x0 - cx) / r, (y0 - cy) / r, (x1 - cx) / r, (y1 - cy) / r)
    if not sweep and dth > 0:
        dth -= 2 * math.pi
    if sweep and dth < 0:
        dth += 2 * math.pi
    return cx, cy, r, a0, a0 + dth


NUM = re.compile(r'[-+]?(?:\d*\.\d+|\d+\.?)(?:[eE][-+]?\d+)?')
FLAG = re.compile(r'[01]')


def tokenize(d):
    """Yield (cmd_letter, [floats]) tuples, splitting arg runs per grammar."""
    toks = re.findall(r'[MmLlHhVvCcSsQqTtAaZz]|' + NUM.pattern, d)
    argc = dict(M=2, L=2, H=1, V=1, C=6, S=4, Q=4, T=2, A=7, Z=0)
    i = 0
    out = []
    while i < len(toks):
        cmd = toks[i]
        i += 1
        n = argc[cmd.upper()]
        if n == 0:
            out.append((cmd, []))
            continue
        first = True
        while i < len(toks) and not toks[i].isalpha():
            args = [float(x) for x in toks[i:i + n]]
            i += n
            out.append((cmd if first else _impl(cmd), args))
            first = False
    return out


def _impl(cmd):
    # A repeated M/m run continues as L/l; others repeat as themselves.
    return {'M': 'L', 'm': 'l'}.get(cmd, cmd)


def flatten_cubic(p0, p1, p2, p3, emit):
    for k in range(1, CUBIC_STEPS + 1):
        t = k / CUBIC_STEPS
        u = 1 - t
        x = u*u*u*p0[0] + 3*u*u*t*p1[0] + 3*u*t*t*p2[0] + t*t*t*p3[0]
        y = u*u*u*p0[1] + 3*u*u*t*p1[1] + 3*u*t*t*p2[1] + t*t*t*p3[1]
        emit(x, y)


def flatten_quad(p0, p1, p2, emit):
    for k in range(1, CUBIC_STEPS + 1):
        t = k / CUBIC_STEPS
        u = 1 - t
        x = u*u*p0[0] + 2*u*t*p1[0] + t*t*p2[0]
        y = u*u*p0[1] + 2*u*t*p1[1] + t*t*p2[1]
        emit(x, y)


def emit_path(d, lines):
    cur = [0.0, 0.0]
    start = [0.0, 0.0]
    prev_ctrl = None  # for S/T reflection
    prev_cmd = None

    def L(x, y):
        lines.append("            sdf.line_to(s * %.4f, s * %.4f)" % (nx(x), nx(y)))
        cur[0], cur[1] = x, y

    for cmd, a in tokenize(d):
        rel = cmd.islower()
        u = cmd.upper()
        if u == 'M':
            x, y = a
            if rel:
                x += cur[0]; y += cur[1]
            lines.append("            sdf.move_to(s * %.4f, s * %.4f)" % (nx(x), nx(y)))
            cur[0], cur[1] = x, y
            start[0], start[1] = x, y
        elif u == 'L':
            x, y = a
            if rel:
                x += cur[0]; y += cur[1]
            L(x, y)
        elif u == 'H':
            x = a[0] + (cur[0] if rel else 0)
            L(x, cur[1])
        elif u == 'V':
            y = a[0] + (cur[1] if rel else 0)
            L(cur[0], y)
        elif u == 'A':
            rx, ry, _rot, large, sweep, x, y = a
            if rel:
                x += cur[0]; y += cur[1]
            cx, cy, r, a0, a1 = arc_centre(cur[0], cur[1], x, y, rx, int(large), int(sweep))
            lines.append("            sdf.arc_to(s * %.4f, s * %.4f, s * %.4f, %.4f, %.4f)"
                         % (nx(cx), nx(cy), nr(r), a0, a1))
            cur[0], cur[1] = x, y
        elif u in ('C', 'S'):
            if u == 'C':
                c1 = (a[0], a[1]); c2 = (a[2], a[3]); end = (a[4], a[5])
                if rel:
                    c1 = (c1[0]+cur[0], c1[1]+cur[1]); c2 = (c2[0]+cur[0], c2[1]+cur[1]); end = (end[0]+cur[0], end[1]+cur[1])
            else:
                c2 = (a[0], a[1]); end = (a[2], a[3])
                if rel:
                    c2 = (c2[0]+cur[0], c2[1]+cur[1]); end = (end[0]+cur[0], end[1]+cur[1])
                if prev_cmd in ('C', 'S') and prev_ctrl:
                    c1 = (2*cur[0]-prev_ctrl[0], 2*cur[1]-prev_ctrl[1])
                else:
                    c1 = (cur[0], cur[1])
            flatten_cubic((cur[0], cur[1]), c1, c2, end, L)
            prev_ctrl = c2
        elif u in ('Q', 'T'):
            if u == 'Q':
                c1 = (a[0], a[1]); end = (a[2], a[3])
                if rel:
                    c1 = (c1[0]+cur[0], c1[1]+cur[1]); end = (end[0]+cur[0], end[1]+cur[1])
            else:
                end = (a[0], a[1])
                if rel:
                    end = (end[0]+cur[0], end[1]+cur[1])
                if prev_cmd in ('Q', 'T') and prev_ctrl:
                    c1 = (2*cur[0]-prev_ctrl[0], 2*cur[1]-prev_ctrl[1])
                else:
                    c1 = (cur[0], cur[1])
            flatten_quad((cur[0], cur[1]), c1, end, L)
            prev_ctrl = c1
        elif u == 'Z':
            L(start[0], start[1])
        prev_cmd = u


def main():
    if len(sys.argv) < 2:
        sys.exit("usage: gen-icon.py <path-to.svg>")
    svg = open(sys.argv[1], encoding='utf-8').read()
    ds = re.findall(r'<path[^>]*\bd="([^"]*)"', svg)
    if not ds:
        sys.exit("no <path d=...> found in " + sys.argv[1])
    lines = ["            let w = s * %.3f" % STROKE_W,
             "            let sdf = Sdf2d.viewport(self.pos * self.rect_size)"]
    for d in ds:
        emit_path(d, lines)
        lines.append("            sdf.stroke(self.color, w)")
    lines.append("            return sdf.result")
    print("\n".join(lines))


if __name__ == '__main__':
    main()
