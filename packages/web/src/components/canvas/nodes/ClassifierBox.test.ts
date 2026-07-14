import { test, expect, vi } from "vitest";
import { render } from "@testing-library/svelte";
import { DEFAULT_DISPLAY, type DiagramDisplay } from "@waml/okf";
import type { OkfNodeData } from "./types";

// NodePorts renders <Handle>, which throws outside a SvelteFlow custom-node
// context. Stub it out so ClassifierBox renders standalone in jsdom.
vi.mock("./NodePorts.svelte", async () => ({ default: (await import("./NodePortsStub.svelte")).default }));

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
