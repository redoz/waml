import { render, fireEvent } from "@testing-library/svelte";
import { describe, it, expect, vi } from "vitest";
import NewPackageDialog from "./NewPackageDialog.svelte";
import type { Template } from "@waml/core/templates";

const templates: Template[] = [
  { id: "t1", nicheId: null, category: "dataset", name: "Orders Domain (UML)", description: "d", bundle: [["orders-domain-uml/order.md", "# Order"]] },
];
const packages = [{ key: "sales" }];

function props(overrides = {}) {
  return { templates, packages, projectName: "My Project", onAdd: vi.fn(), onClose: vi.fn(), ...overrides };
}

describe("NewPackageDialog", () => {
  it("defaults to the Empty package starter with name 'New package'", () => {
    const { getByLabelText } = render(NewPackageDialog, { props: props() });
    expect((getByLabelText("Package name") as HTMLInputElement).value).toBe("New package");
  });

  it("lists the blank, the four diagram kinds and the templates as one flat card list", () => {
    const { getByText } = render(NewPackageDialog, { props: props() });
    expect(getByText("Start from")).toBeTruthy();
    expect(getByText("Empty package")).toBeTruthy();
    expect(getByText("Domain model")).toBeTruthy();
    expect(getByText("Use-case")).toBeTruthy();
    expect(getByText("Activity")).toBeTruthy();
    expect(getByText("Sequence")).toBeTruthy();
    expect(getByText("Orders Domain (UML)")).toBeTruthy();
  });

  it("picking a diagram card defaults the name to that kind", async () => {
    const { getByText, getByLabelText } = render(NewPackageDialog, { props: props() });
    await fireEvent.click(getByText("Activity"));
    expect((getByLabelText("Package name") as HTMLInputElement).value).toBe("Activity");
  });

  it("blocks Add on a name collision with an inline message", async () => {
    // 'sales' already exists at root; typing it must disable Add.
    const { getByLabelText, getByText, getByRole } = render(NewPackageDialog, { props: props() });
    const input = getByLabelText("Package name") as HTMLInputElement;
    await fireEvent.input(input, { target: { value: "Sales" } });
    expect(getByText("name already used here")).toBeTruthy();
    expect((getByRole("button", { name: "Add" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("the Empty package card emits an empty-tier payload with the selected parent and name", async () => {
    const onAdd = vi.fn();
    const { getByLabelText, getByRole } = render(NewPackageDialog, { props: props({ onAdd }) });
    await fireEvent.input(getByLabelText("Package name"), { target: { value: "Fresh" } });
    await fireEvent.click(getByRole("button", { name: "Add" }));
    expect(onAdd).toHaveBeenCalledWith({ tier: "empty", parentPath: "", name: "Fresh" });
  });

  it("a template card emits the chosen template's bundle", async () => {
    const onAdd = vi.fn();
    const { getByText, getByRole } = render(NewPackageDialog, { props: props({ onAdd }) });
    await fireEvent.click(getByText("Orders Domain (UML)"));
    await fireEvent.click(getByRole("button", { name: "Add" }));
    expect(onAdd).toHaveBeenCalledWith(expect.objectContaining({ tier: "template", parentPath: "", bundle: templates[0].bundle }));
  });

  it("Place in is a native select of the project root and packages", () => {
    const { getByLabelText } = render(NewPackageDialog, { props: props() });
    const select = getByLabelText("Place in") as HTMLSelectElement;
    const opts = [...select.options].map((o) => o.textContent?.trim());
    expect(opts).toContain("My Project");
    expect(opts).toContain("sales");
    expect(select.value).toBe(""); // defaults to root
  });

  it("choosing a package in Place in threads it into the payload", async () => {
    const onAdd = vi.fn();
    const { getByLabelText, getByRole } = render(NewPackageDialog, { props: props({ onAdd }) });
    await fireEvent.change(getByLabelText("Place in"), { target: { value: "sales" } });
    await fireEvent.input(getByLabelText("Package name"), { target: { value: "Fresh" } });
    await fireEvent.click(getByRole("button", { name: "Add" }));
    expect(onAdd).toHaveBeenCalledWith({ tier: "empty", parentPath: "sales", name: "Fresh" });
  });
});
