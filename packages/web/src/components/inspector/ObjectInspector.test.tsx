import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { ObjectInspector } from "./ObjectInspector";
import type { ModelNode } from "@mc/okf";

const node: ModelNode = { key: "n1", type: "uml.Class", title: "Order", stereotypes: [], attributes: [], position: { x: 0, y: 0 } };

describe("ObjectInspector palette", () => {
  it("offers the profile's metaclasses in the type datalist", () => {
    const { container } = render(<ObjectInspector node={node} onUpdate={() => {}} profileName="uml-domain" />);
    const options = [...container.querySelectorAll("datalist#okf-metaclasses option")].map(o => o.getAttribute("value"));
    expect(options).toEqual(["uml.Class", "uml.Interface", "uml.Enum", "uml.DataType"]);
  });
  it("offers the profile's stereotypes in a datalist", () => {
    const { container } = render(<ObjectInspector node={node} onUpdate={() => {}} profileName="uml-domain" />);
    const options = [...container.querySelectorAll("datalist#okf-stereotypes option")].map(o => o.getAttribute("value"));
    expect(options).toContain("aggregateRoot");
  });
  it("switching type to uml.Enum shows the values editor", () => {
    const onUpdate = vi.fn();
    render(<ObjectInspector node={{ ...node, type: "uml.Enum", values: ["A"] }} onUpdate={onUpdate} profileName="uml-domain" />);
    expect(screen.getByText("Values (one per line)")).toBeTruthy();
  });
});
