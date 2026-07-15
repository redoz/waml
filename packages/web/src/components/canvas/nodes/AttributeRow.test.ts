import { test, expect } from "vitest";
import { render } from "@testing-library/svelte";
import AttributeRow from "./AttributeRow.svelte";
import type { Attribute } from "@waml/okf";

const attr = (over: Partial<Attribute> = {}): Attribute =>
  ({ name: "id", type: { name: "STRING" }, multiplicity: "0..*", visibility: "+", ...over }) as Attribute;

test("showTypes shows the type name; showMultiplicity shows the {mult} suffix", () => {
  const { container } = render(AttributeRow, { props: { a: attr(), showTypes: true, showMultiplicity: true } });
  expect(container.textContent).toContain("STRING");
  expect(container.textContent).toContain("{0..*}");
});

test("multiplicity is independent of type name (name-only still shows {mult})", () => {
  const { container } = render(AttributeRow, { props: { a: attr(), showTypes: false, showMultiplicity: true } });
  expect(container.textContent).not.toContain("STRING");
  expect(container.textContent).toContain("{0..*}");
});

test("showMultiplicity off drops the suffix; showTypes on keeps the type", () => {
  const { container } = render(AttributeRow, { props: { a: attr(), showTypes: true, showMultiplicity: false } });
  expect(container.textContent).toContain("STRING");
  expect(container.textContent).not.toContain("{0..*}");
});

test("both off renders no trailing type/mult column", () => {
  const { container } = render(AttributeRow, { props: { a: attr(), showTypes: false, showMultiplicity: false } });
  expect(container.querySelector("span.font-mono.text-\\[10\\.5px\\]")).toBeNull();
});

test("multiplicity of exactly '1' is never printed", () => {
  const { container } = render(AttributeRow, {
    props: { a: attr({ multiplicity: "1" }), showTypes: false, showMultiplicity: true },
  });
  expect(container.textContent).not.toContain("{1}");
});

test("visibility marker gated by showVisibility", () => {
  const on = render(AttributeRow, { props: { a: attr(), showVisibility: true } });
  expect(on.container.textContent).toContain("+");
  const off = render(AttributeRow, { props: { a: attr(), showVisibility: false } });
  expect(off.container.querySelector("span.font-mono")?.textContent).not.toContain("+");
});
