import { test, expect, vi } from "vitest";
import { render } from "@testing-library/svelte";
import { DEFAULT_DISPLAY, type DiagramDisplay } from "@waml/okf";
import type { OkfNodeData } from "./types";

// NodePorts renders <Handle>, which throws outside a SvelteFlow custom-node
// context. Stub it out so ClassifierBox renders standalone in jsdom.
vi.mock("./NodePorts.svelte", async () => ({ default: (await import("./NodePortsStub.svelte")).default }));

// The only registered profile (uml-domain) always floors visibility off
// (hide: ["operations", "visibility"]), so the "profile allows visibility"
// half of ClassifierBox's `showVisibility` AND-gate is otherwise untested.
// Partial-mock getProfile so a synthetic "_profile" name resolves to a
// uml-domain-derived profile with an empty `hide` list, while keeping the
// real `stereotypeStyle` implementation intact.
vi.mock("@waml/core/profiles", async () => {
  const actual = await vi.importActual<typeof import("@waml/core/profiles")>("@waml/core/profiles");
  return {
    ...actual,
    getProfile: (name?: string) =>
      name === "mock-visibility-open" ? { ...actual.getProfile("uml-domain"), hide: [] } : actual.getProfile(name),
  };
});

const ClassifierBox = (await import("./ClassifierBox.svelte")).default;

const mkData = (display: DiagramDisplay): OkfNodeData =>
  ({
    concept: { id: "n", type: "uml.Class", title: "Order", body: "" },
    key: "n",
    type: "uml.Class",
    stereotypes: ["entity"],
    attributes: [
      { name: "id", type: { name: "STRING" }, multiplicity: "1" },
      { name: "total", type: { name: "MONEY" }, multiplicity: "1" },
    ],
    position: { x: 0, y: 0 },
    _display: display,
    _profile: "uml-domain",
  }) as OkfNodeData;

const disp = (over: Partial<DiagramDisplay>): DiagramDisplay => ({ ...DEFAULT_DISPLAY, ...over });

test("showAttributes on renders attribute rows; off collapses to a count", () => {
  const shown = render(ClassifierBox, { props: { data: mkData(disp({ showAttributes: true })) } });
  expect(shown.container.textContent).toContain("id");
  expect(shown.container.textContent).toContain("total");
  expect(shown.container.textContent).not.toContain("2 attributes");

  const hidden = render(ClassifierBox, { props: { data: mkData(disp({ showAttributes: false })) } });
  expect(hidden.container.textContent).toContain("2 attributes");
});

test("attributeDetail name-type shows the type column; name-only hides it", () => {
  const nameType = render(ClassifierBox, { props: { data: mkData(disp({ showAttributes: true, attributeDetail: "name-type" })) } });
  expect(nameType.container.textContent).toContain("STRING");
  expect(nameType.container.textContent).toContain("MONEY");

  const nameOnly = render(ClassifierBox, { props: { data: mkData(disp({ showAttributes: true, attributeDetail: "name-only" })) } });
  expect(nameOnly.container.textContent).toContain("id");
  expect(nameOnly.container.textContent).not.toContain("STRING");
  expect(nameOnly.container.textContent).not.toContain("MONEY");
});

test("showStereotype toggles the «stereotype» row", () => {
  const on = render(ClassifierBox, { props: { data: mkData(disp({ showStereotype: true })) } });
  expect(on.container.textContent).toContain("«entity»");

  const off = render(ClassifierBox, { props: { data: mkData(disp({ showStereotype: false })) } });
  expect(off.container.textContent).not.toContain("«entity»");
});

const mkAttrData = (display: DiagramDisplay, profile = "uml-domain"): OkfNodeData =>
  ({
    concept: { id: "n", type: "uml.Class", title: "Order", body: "" },
    key: "n",
    type: "uml.Class",
    stereotypes: ["entity"],
    attributes: [{ name: "id", type: { name: "STRING" }, multiplicity: "0..*", visibility: "+" }],
    position: { x: 0, y: 0 },
    _display: display,
    _profile: profile,
  }) as OkfNodeData;

test("uml-domain hides visibility as a floor even when showAttributeVisibility is true", () => {
  const { container } = render(ClassifierBox, {
    props: { data: mkAttrData(disp({ showAttributes: true, showAttributeVisibility: true })) },
  });
  // marker "+" must not appear as an attribute-visibility glyph
  expect(container.querySelector(".relative.flex span.font-mono")?.textContent ?? "").not.toContain("+");
});

test("showAttributeMultiplicity drives the {mult} suffix independent of attributeDetail", () => {
  const shown = render(ClassifierBox, {
    props: { data: mkAttrData(disp({ showAttributes: true, attributeDetail: "name-only", showAttributeMultiplicity: true })) },
  });
  expect(shown.container.textContent).toContain("{0..*}");

  const hidden = render(ClassifierBox, {
    props: { data: mkAttrData(disp({ showAttributes: true, attributeDetail: "name-type", showAttributeMultiplicity: false })) },
  });
  expect(hidden.container.textContent).toContain("STRING");
  expect(hidden.container.textContent).not.toContain("{0..*}");
});

test("a profile that allows visibility still requires showAttributeVisibility (AND, not profile-only)", () => {
  const on = render(ClassifierBox, {
    props: { data: mkAttrData(disp({ showAttributes: true, showAttributeVisibility: true }), "mock-visibility-open") },
  });
  expect(on.container.querySelector(".relative.flex span.font-mono")?.textContent ?? "").toContain("+");

  const off = render(ClassifierBox, {
    props: { data: mkAttrData(disp({ showAttributes: true, showAttributeVisibility: false }), "mock-visibility-open") },
  });
  expect(off.container.querySelector(".relative.flex span.font-mono")?.textContent ?? "").not.toContain("+");
});

const mkManyAttrs = (display: DiagramDisplay): OkfNodeData =>
  ({
    concept: { id: "n", type: "uml.Class", title: "Order", body: "" },
    key: "n",
    type: "uml.Class",
    stereotypes: ["entity"],
    attributes: [
      { name: "a1", type: { name: "STRING" }, multiplicity: "1" },
      { name: "a2", type: { name: "STRING" }, multiplicity: "1" },
      { name: "a3", type: { name: "STRING" }, multiplicity: "1" },
      { name: "a4", type: { name: "STRING" }, multiplicity: "1" },
    ],
    position: { x: 0, y: 0 },
    _display: display,
    _profile: "uml-domain",
  }) as OkfNodeData;

test("maxAttributes caps attribute rows with a static '+K more' footer, no expand button", () => {
  const { container } = render(ClassifierBox, {
    props: { data: mkManyAttrs(disp({ showAttributes: true, maxAttributes: 2 })) },
  });
  expect(container.textContent).toContain("a1");
  expect(container.textContent).toContain("a2");
  expect(container.textContent).not.toContain("a3");
  expect(container.textContent).toContain("+2 more");
  expect(container.querySelector("button")).toBeNull();

  const { container: uncapped } = render(ClassifierBox, {
    props: { data: mkManyAttrs(disp({ showAttributes: true })) },
  });
  expect(uncapped.textContent).toContain("a4");
});

const mkTags = (display: DiagramDisplay): OkfNodeData =>
  ({
    concept: { id: "n", type: "uml.Class", title: "Order", body: "" },
    key: "n", type: "uml.Class", stereotypes: ["entity", "valueObject"],
    attributes: [], position: { x: 0, y: 0 }, _display: display, _profile: "uml-domain",
  }) as OkfNodeData;

test("undefined filter shows every stereotype tag", () => {
  const { container } = render(ClassifierBox, { props: { data: mkTags(disp({ showStereotype: true, stereotypeFilter: undefined })) } });
  expect(container.textContent).toContain("«entity»");
  expect(container.textContent).toContain("«valueObject»");
});

test("an allowlist shows only listed tags", () => {
  const { container } = render(ClassifierBox, { props: { data: mkTags(disp({ showStereotype: true, stereotypeFilter: ["entity"] })) } });
  expect(container.textContent).toContain("«entity»");
  expect(container.textContent).not.toContain("«valueObject»");
});

test("empty allowlist shows no tags but keeps the keyword row", () => {
  const { container } = render(ClassifierBox, { props: { data: mkTags(disp({ showStereotype: true, stereotypeFilter: [] })), keyword: "Class" } });
  expect(container.textContent).toContain("«Class»");
  expect(container.textContent).not.toContain("«entity»");
});

test("showStereotype false renders neither keyword nor tags", () => {
  const { container } = render(ClassifierBox, { props: { data: mkTags(disp({ showStereotype: false })), keyword: "Class" } });
  expect(container.textContent).not.toContain("«Class»");
  expect(container.textContent).not.toContain("«entity»");
});

const style = (container: HTMLElement) => (container.firstElementChild as HTMLElement).getAttribute("style") ?? "";

// Note: jsdom (via the `cssstyle` package) normalizes recognized color
// declarations set through `element.style.cssText` — which is how Svelte 5
// applies a dynamic `style={...}` attribute — into `rgb(r, g, b)` form
// before reflecting them back through `getAttribute("style")`. So these
// assertions match on the normalized rgb() form rather than the literal hex
// input, even though we're reading the raw attribute string (not the CSSOM)
// per the jsdom + color-mix() note above.
test("a stereotype color overrides the header accent and adds a wash", () => {
  const { container } = render(ClassifierBox, {
    props: { data: mkTags(disp({ showStereotype: true, stereotypeColors: { entity: "#ff0000" } })) },
  });
  const s = style(container);
  expect(s).toContain("border-top-color: rgb(255, 0, 0)");
  expect(s).toContain("color-mix(in srgb, rgb(255, 0, 0) 12%, white)");
});

test("override color follows later-wins precedence across stereotypes", () => {
  const { container } = render(ClassifierBox, {
    props: { data: mkTags(disp({ showStereotype: true, stereotypeColors: { entity: "#111111", valueObject: "#222222" } })) },
  });
  // data.stereotypes = ["entity", "valueObject"] ⇒ valueObject wins
  expect(style(container)).toContain("border-top-color: rgb(34, 34, 34)");
});

test("no override keeps the plain white background (no wash)", () => {
  const { container } = render(ClassifierBox, {
    props: { data: mkTags(disp({ showStereotype: true, stereotypeColors: {} })) },
  });
  expect(style(container)).not.toContain("color-mix");
});
