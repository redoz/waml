#!/usr/bin/env python3
"""Generate the IconPin body (crates/waml-editor/src/icons.rs) from pin.svg.

The Lucide pushpin is straight runs plus seven circular `a` fillets and a cap.
All arcs are rx==ry, so each maps to one `sdf.arc_to(cx, cy, r, a0, a1)` centre-
form call (a0/a1 radians, 0 = +x, ccw+). This script does the SVG endpoint->centre
conversion offline and applies the cell fit, so icons.rs carries no hand-flattened
polyline. Angles are y-down screen space (matching the shader's atan2).

Fit: norm(c) = A*c + B, uniform + isotropic (A scales radii, angles pass through).
A/B below place the pin centred in the cell with the stroke held inside the
Sdf2d.viewport clip (DrawSvg would instead bleed its stroke outside its rect).

Run:  python scripts/gen-pin-icon.py   # prints the DSL body to stdout
"""
import math

# net cell fit (see module note): 0.045*c - 0.04 (24-space -> cell), then *0.9504
# about the centre 0.5 to keep stroke w=0.068 inside the viewport clip.
A = 0.042768
B = -0.013216
STROKE_W = 0.068  # half-width; Sdf2d.stroke treats w as half-width


def arc_params(p0, p1, r, large, sweep):
    """SVG circular-arc endpoint form -> (end, cx, cy, r_corrected, a0, a1)."""
    x0, y0 = p0
    x1, y1 = p1
    dx = (x0 - x1) / 2
    dy = (y0 - y1) / 2
    rc = (dx * dx) / (r * r) + (dy * dy) / (r * r)
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
        a = math.acos(max(-1, min(1, dot / ln)))
        return -a if ux * vy - uy * vx < 0 else a

    th0 = ang(1, 0, (x0 - cx) / r, (y0 - cy) / r)
    dth = ang((x0 - cx) / r, (y0 - cy) / r, (x1 - cx) / r, (y1 - cy) / r)
    if not sweep and dth > 0:
        dth -= 2 * math.pi
    if sweep and dth < 0:
        dth += 2 * math.pi
    return (x1, y1, cx, cy, r, th0, th0 + dth)


ops = []
cur = [9, 10.76]
ops.append(("move", cur[0], cur[1]))


def A_cmd(x, y, r, large, sweep):
    ex, ey, cx, cy, rr, a0, a1 = arc_params(tuple(cur), (x, y), r, large, sweep)
    ops.append(("arc", cx, cy, rr, a0, a1))
    cur[0], cur[1] = ex, ey


def L_cmd(x, y):
    ops.append(("line", x, y))
    cur[0], cur[1] = x, y


# pin.svg body path (viewBox 24) transcribed command-for-command
A_cmd(9 - 1.11, 10.76 + 1.79, 2, 0, 1)
L_cmd(9 - 1.11 - 1.78, 12.55 + 0.9)
A_cmd(5, 15.24, 2, 0, 0)
L_cmd(5, 16)
A_cmd(6, 17, 1, 0, 0)
L_cmd(18, 17)
A_cmd(19, 16, 1, 0, 0)
L_cmd(19, 15.24)
A_cmd(19 - 1.11, 15.24 - 1.79, 2, 0, 0)
L_cmd(19 - 1.11 - 1.78, 13.45 - 0.9)
A_cmd(15, 10.76, 2, 0, 1)
L_cmd(15, 7)
A_cmd(16, 6, 1, 0, 1)
A_cmd(16, 2, 2, 0, 0)
L_cmd(8, 2)
A_cmd(8, 6, 2, 0, 0)
A_cmd(9, 7, 1, 0, 1)
L_cmd(9, 10.76)  # close


def nx(c):
    return A * c + B


def nr(r):
    return A * r


print("            let w = s * %.3f" % STROKE_W)
print("            let sdf = Sdf2d.viewport(self.pos * self.rect_size)")
for op in ops:
    if op[0] == "move":
        print("            sdf.move_to(s * %.4f, s * %.4f)" % (nx(op[1]), nx(op[2])))
    elif op[0] == "line":
        print("            sdf.line_to(s * %.4f, s * %.4f)" % (nx(op[1]), nx(op[2])))
    else:
        _, cx, cy, r, a0, a1 = op
        print("            sdf.arc_to(s * %.4f, s * %.4f, s * %.4f, %.4f, %.4f)"
              % (nx(cx), nx(cy), nr(r), a0, a1))
print("            sdf.stroke(self.color, w)")
print("            sdf.move_to(s * %.4f, s * %.4f)" % (nx(12), nx(17)))  # needle
print("            sdf.line_to(s * %.4f, s * %.4f)" % (nx(12), nx(22)))
print("            sdf.stroke(self.color, w)")
