import { test, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import type { Attribute } from "@waml/okf";
import AttributeEditor from "./AttributeEditor.svelte";

const attrs: Attribute[] = [{ name: "id", type: { name: "String" }, multiplicity: "1" }];

test("editing a name calls onChange with the patched row", async () => {
  const onChange = vi.fn();
  render(AttributeEditor, { props: { attributes: attrs, onChange } });
  const nameInput = screen.getByPlaceholderText("name") as HTMLInputElement;
  await fireEvent.input(nameInput, { target: { value: "orderId" } });
  expect(onChange).toHaveBeenCalledWith([
    expect.objectContaining({ name: "orderId", type: { name: "String" }, multiplicity: "1" }),
  ]);
});

test("Add attribute appends a row with implicit multiplicity", async () => {
  const onChange = vi.fn();
  render(AttributeEditor, { props: { attributes: attrs, onChange } });
  await fireEvent.click(screen.getByRole("button", { name: /Add attribute/ }));
  expect(onChange).toHaveBeenCalledWith([
    attrs[0],
    { name: "", type: { name: "String" } },
  ]);
});

test("blank multiplicity input authors undefined", async () => {
  const onChange = vi.fn();
  render(AttributeEditor, { props: { attributes: attrs, onChange } });
  const multiplicityInput = screen.getByPlaceholderText("1") as HTMLInputElement;
  await fireEvent.input(multiplicityInput, { target: { value: "" } });
  expect(onChange).toHaveBeenCalledWith([
    { name: "id", type: { name: "String" }, multiplicity: undefined },
  ]);
});

test("typed default multiplicity remains explicitly authored", async () => {
  const onChange = vi.fn();
  const implicitAttrs: Attribute[] = [{ name: "id", type: { name: "String" } }];
  render(AttributeEditor, { props: { attributes: implicitAttrs, onChange } });
  const multiplicityInput = screen.getByPlaceholderText("1") as HTMLInputElement;
  await fireEvent.input(multiplicityInput, { target: { value: "1" } });
  expect(onChange).toHaveBeenCalledWith([
    { name: "id", type: { name: "String" }, multiplicity: "1" },
  ]);
});
