import { test, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import type { Attribute } from "@uaml/okf";
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

test("Add attribute appends a default row", async () => {
  const onChange = vi.fn();
  render(AttributeEditor, { props: { attributes: attrs, onChange } });
  await fireEvent.click(screen.getByRole("button", { name: /Add attribute/ }));
  expect(onChange).toHaveBeenCalledWith([
    attrs[0],
    { name: "", type: { name: "String" }, multiplicity: "1" },
  ]);
});
