import { test, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import DiagramPropertiesBody from "./DiagramPropertiesBody.svelte";
import { DEFAULT_DISPLAY } from "@uaml/okf";

const props = (over = {}) => ({
  display: { ...DEFAULT_DISPLAY },
  onChange: vi.fn(),
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
