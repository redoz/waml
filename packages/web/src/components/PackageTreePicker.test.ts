import { render, fireEvent } from "@testing-library/svelte";
import { describe, it, expect, vi } from "vitest";
import PackageTreePicker from "./PackageTreePicker.svelte";

const packages = [{ key: "sales" }, { key: "sales/orders" }, { key: "billing" }];

describe("PackageTreePicker", () => {
  it("renders the project root and every package", () => {
    const { getByText } = render(PackageTreePicker, {
      props: { packages, projectName: "My Project", selected: "", onSelect: () => {} },
    });
    expect(getByText("My Project")).toBeTruthy();
    expect(getByText("orders")).toBeTruthy();
    expect(getByText("billing")).toBeTruthy();
  });

  it("selecting a package reports its key; selecting root reports empty string", async () => {
    const onSelect = vi.fn();
    const { getByText } = render(PackageTreePicker, {
      props: { packages, projectName: "My Project", selected: "", onSelect },
    });
    await fireEvent.click(getByText("orders"));
    expect(onSelect).toHaveBeenCalledWith("sales/orders");
    await fireEvent.click(getByText("My Project"));
    expect(onSelect).toHaveBeenCalledWith("");
  });
});
