export function slugify(text: string, fallback = ""): string {
  const s = (text || "")
    .replace(/([a-z0-9])([A-Z])/g, "$1-$2")      // camelCase boundary: OrderStatus → Order-Status
    .replace(/([A-Z]+)([A-Z][a-z])/g, "$1-$2")   // acronym boundary: HTTPServer → HTTP-Server
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
  return s || fallback;
}
export function renderFrontmatter(obj: Record<string, unknown>, indent = ""): string {
  const lines: string[] = [];
  for (const [k, v] of Object.entries(obj)) {
    if (v === null || v === undefined) continue;
    if (Array.isArray(v)) lines.push(`${indent}${k}: [${v.map(scalar).join(", ")}]`);
    else if (typeof v === "object") {
      const entries = Object.entries(v as Record<string, unknown>);
      const allScalar = entries.every(([, x]) => typeof x !== "object" || x === null);
      if (allScalar && entries.length <= 2 && entries.every(([, x]) => typeof x === "number"))
        lines.push(`${indent}${k}: { ${entries.map(([ek, ev]) => `${ek}: ${ev}`).join(", ")} }`);
      else { lines.push(`${indent}${k}:`); lines.push(renderFrontmatter(v as Record<string, unknown>, indent + "  ")); }
    } else lines.push(`${indent}${k}: ${scalar(v)}`);
  }
  return lines.join("\n");
}
function scalar(v: unknown): string {
  if (typeof v === "number" || typeof v === "boolean") return String(v);
  return `"${String(v).replace(/\\/g, "\\\\").replace(/"/g, '\\"')}"`;
}
// Values a hand-rolled frontmatter block can hold: scalars, nested blocks,
// and flat lists thereof (mirrors what renderFrontmatter/parseValue below
// actually produce; this parser is not a general YAML implementation).
type FrontmatterValue =
  | string
  | number
  | boolean
  | FrontmatterValue[]
  | { [key: string]: FrontmatterValue };
type FrontmatterObject = Record<string, FrontmatterValue>;

export function parseFrontmatter(text: string): { data: FrontmatterObject; body: string } {
  const m = text.match(/^---\n([\s\S]*?)\n---\n?([\s\S]*)$/);
  if (!m) return { data: {}, body: text };
  return { data: parseYaml(m[1]), body: m[2] };
}
function parseYaml(src: string): FrontmatterObject {
  const root: FrontmatterObject = {};
  const stack: { indent: number; obj: FrontmatterObject }[] = [{ indent: -1, obj: root }];
  for (const raw of src.split("\n")) {
    if (!raw.trim()) continue;
    const indent = raw.match(/^ */)![0].length;
    const line = raw.trim(); const ci = line.indexOf(":"); if (ci < 0) continue;
    const key = line.slice(0, ci).trim(); const rest = line.slice(ci + 1).trim();
    while (stack.length && indent <= stack[stack.length - 1].indent) stack.pop();
    const parent = stack[stack.length - 1].obj;
    if (rest === "") { const obj: FrontmatterObject = {}; parent[key] = obj; stack.push({ indent, obj }); }
    else parent[key] = parseValue(rest);
  }
  return root;
}
function parseValue(s: string): FrontmatterValue {
  if (s.startsWith("[")) return s.slice(1, -1).split(",").map(x => parseValue(x.trim())).filter(x => x !== "");
  if (s.startsWith("{")) { const o: Record<string, number> = {};
    s.slice(1, -1).split(",").forEach(p => { const [k, v] = p.split(":").map(x => x.trim()); if (k) o[k] = Number(v); }); return o; }
  if (s.startsWith('"')) return s.slice(1, -1).replace(/\\"/g, '"').replace(/\\\\/g, "\\");
  if (/^-?\d+(\.\d+)?$/.test(s)) return Number(s);
  if (s === "true" || s === "false") return s === "true";
  return s;
}
