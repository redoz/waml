import { test, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import DiagramPropertiesBody from "./DiagramPropertiesBody.svelte";
import { DEFAULT_DISPLAY, type Diagram } from "@waml/okf";

const diagram: Diagram = { key: "orders", title: "Orders", profile: "uml-domain", members: [] };

const props = (over = {}) => ({
  display: { ...DEFAULT_DISPLAY },
  diagram,
  candidateStereotypes: [] as string[],
  editable: true,
  onChange: vi.fn(),
  onUpdateDiagram: vi.fn(),
  ...over,
});

test("renders all five display controls", () => {
  render(DiagramPropertiesBody, { props: props() });
  expect(screen.getByRole("switch", { name: "Show attributes" })).toBeTruthy();
  expect(screen.getByRole("radiogroup", { name: "Attribute detail" })).toBeTruthy();
  expect(screen.getByRole("radiogroup", { name: "Associations" })).toBeTruthy();
  expect(screen.getByRole("switch", { name: "Emphasize multiplicity" })).toBeTruthy();
  expect(screen.getByRole("switch", { name: "Show stereotype" })).toBeTruthy();
});

test("toggling 'Show attributes' emits the inverted flag", async () => {
  const onChange = vi.fn();
  render(DiagramPropertiesBody, {
    props: props({ display: { ...DEFAULT_DISPLAY, showAttributes: true }, onChange }),
  });
  await fireEvent.click(screen.getByRole("switch", { name: "Show attributes" }));
  expect(onChange).toHaveBeenCalledWith({ showAttributes: false });
});

test("picking an attribute-detail option emits that value", async () => {
  const onChange = vi.fn();
  render(DiagramPropertiesBody, {
    props: props({ display: { ...DEFAULT_DISPLAY, showAttributes: true }, onChange }),
  });
  await fireEvent.click(screen.getByRole("radio", { name: "Name + type" }));
  expect(onChange).toHaveBeenCalledWith({ attributeDetail: "name-type" });
});

test("a non-default display value drives every control's rendered state", () => {
  const display = {
    ...DEFAULT_DISPLAY,
    showAttributes: false,
    attributeDetail: "name-only" as const,
    associationLabels: "hidden" as const,
    emphasizeMultiplicity: true,
    showStereotype: false,
  };
  render(DiagramPropertiesBody, { props: props({ display }) });

  expect(screen.getByRole("switch", { name: "Show attributes" }).getAttribute("aria-checked")).toBe(
    "false",
  );
  expect(
    screen.getByRole("switch", { name: "Emphasize multiplicity" }).getAttribute("aria-checked"),
  ).toBe("true");
  expect(screen.getByRole("switch", { name: "Show stereotype" }).getAttribute("aria-checked")).toBe(
    "false",
  );

  expect(screen.getByRole("radio", { name: "Name only" }).getAttribute("aria-checked")).toBe("true");
  expect(screen.getByRole("radio", { name: "Name + type" }).getAttribute("aria-checked")).toBe(
    "false",
  );

  expect(screen.getByRole("radio", { name: "Hide labels" }).getAttribute("aria-checked")).toBe("true");
  expect(screen.getByRole("radio", { name: "Show labels" }).getAttribute("aria-checked")).toBe(
    "false",
  );
});

test("picking an associations option emits that value", async () => {
  const onChange = vi.fn();
  render(DiagramPropertiesBody, { props: props({ onChange }) });
  await fireEvent.click(screen.getByRole("radio", { name: "Hide labels" }));
  expect(onChange).toHaveBeenCalledWith({ associationLabels: "hidden" });
});

test("attribute-detail options are disabled and inert when 'Show attributes' is off", async () => {
  const onChange = vi.fn();
  render(DiagramPropertiesBody, {
    props: props({ display: { ...DEFAULT_DISPLAY, showAttributes: false }, onChange }),
  });

  const nameOnly = screen.getByRole("radio", { name: "Name only" }) as HTMLButtonElement;
  const nameType = screen.getByRole("radio", { name: "Name + type" }) as HTMLButtonElement;
  expect(nameOnly.disabled).toBe(true);
  expect(nameType.disabled).toBe(true);

  await fireEvent.click(nameOnly);
  expect(onChange).not.toHaveBeenCalled();
});

test("editing the title commits on blur via onUpdateDiagram", async () => {
  const onUpdateDiagram = vi.fn();
  render(DiagramPropertiesBody, { props: props({ onUpdateDiagram }) });
  const input = screen.getByLabelText("Diagram title") as HTMLInputElement;
  await fireEvent.input(input, { target: { value: "Order lifecycle" } });
  await fireEvent.blur(input);
  expect(onUpdateDiagram).toHaveBeenCalledWith({ title: "Order lifecycle" });
});

test("editing the note commits on blur via onUpdateDiagram", async () => {
  const onUpdateDiagram = vi.fn();
  render(DiagramPropertiesBody, { props: props({ onUpdateDiagram }) });
  const note = screen.getByLabelText("Diagram note") as HTMLTextAreaElement;
  await fireEvent.input(note, { target: { value: "Notes for reviewers" } });
  await fireEvent.blur(note);
  expect(onUpdateDiagram).toHaveBeenCalledWith({ description: "Notes for reviewers" });
});

test("Show visibility toggle emits showAttributeVisibility", async () => {
  const onChange = vi.fn();
  render(DiagramPropertiesBody, { props: props({ display: { ...DEFAULT_DISPLAY, showAttributes: true, showAttributeVisibility: true }, onChange }) });
  await fireEvent.click(screen.getByRole("switch", { name: "Show visibility" }));
  expect(onChange).toHaveBeenCalledWith({ showAttributeVisibility: false });
});

test("Show multiplicity toggle emits showAttributeMultiplicity", async () => {
  const onChange = vi.fn();
  render(DiagramPropertiesBody, { props: props({ display: { ...DEFAULT_DISPLAY, showAttributes: true, showAttributeMultiplicity: true }, onChange }) });
  await fireEvent.click(screen.getByRole("switch", { name: "Show multiplicity" }));
  expect(onChange).toHaveBeenCalledWith({ showAttributeMultiplicity: false });
});

test("Max attributes: typing a number emits it; Unlimited emits undefined", async () => {
  const onChange = vi.fn();
  render(DiagramPropertiesBody, { props: props({ display: { ...DEFAULT_DISPLAY, showAttributes: true }, onChange }) });
  await fireEvent.input(screen.getByLabelText("Max attributes"), { target: { value: "6" } });
  expect(onChange).toHaveBeenCalledWith({ maxAttributes: 6 });
  await fireEvent.click(screen.getByRole("button", { name: "Unlimited attributes" }));
  expect(onChange).toHaveBeenCalledWith({ maxAttributes: undefined });
});

test("editable false shows the banner and disables every control", async () => {
  const onChange = vi.fn();
  const onUpdateDiagram = vi.fn();
  render(DiagramPropertiesBody, { props: props({ editable: false, onChange, onUpdateDiagram }) });
  expect(screen.getByRole("note")).toBeTruthy();
  const showAttrs = screen.getByRole("switch", { name: "Show attributes" }) as HTMLButtonElement;
  expect(showAttrs.disabled).toBe(true);
  await fireEvent.click(showAttrs);
  expect(onChange).not.toHaveBeenCalled();
  const title = screen.getByLabelText("Diagram title") as HTMLInputElement;
  expect(title.disabled).toBe(true);
});
