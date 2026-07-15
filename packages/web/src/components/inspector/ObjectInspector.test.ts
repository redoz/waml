import { describe, it, test, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import type { ModelNode, ModelEdge } from "@waml/okf";
import ObjectInspector from "./ObjectInspector.svelte";

const node: ModelNode = {
  concept: { id: "n1", type: "uml.Class", title: "Order", body: "" },
  key: "n1",
  type: "uml.Class",
  stereotypes: [],
  attributes: [],
  position: { x: 0, y: 0 },
};

test("editing title patches the concept through onUpdate", async () => {
  const onUpdate = vi.fn();
  render(ObjectInspector, { props: { node, onUpdate, profileName: "uml-domain" } });
  const input = screen.getByDisplayValue("Order") as HTMLInputElement;
  await fireEvent.input(input, { target: { value: "Orders" } });
  expect(onUpdate).toHaveBeenCalledWith({ concept: { ...node.concept, title: "Orders" } });
});

test("description field is sourced from concept.description", () => {
  // The display reads `concept.description` — the single authoritative source.
  const withDesc: ModelNode = {
    ...node,
    concept: { ...node.concept, description: "From concept" },
  };
  render(ObjectInspector, { props: { node: withDesc, onUpdate: () => {}, profileName: "uml-domain" } });
  expect(screen.getByDisplayValue("From concept")).toBeTruthy();
});

test("editing the description patches the concept through onUpdate", async () => {
  const onUpdate = vi.fn();
  render(ObjectInspector, { props: { node, onUpdate, profileName: "uml-domain" } });
  const textarea = screen.getByLabelText("Description") as HTMLTextAreaElement;
  await fireEvent.input(textarea, { target: { value: "Placed by a customer" } });
  expect(onUpdate).toHaveBeenCalledWith({ concept: { ...node.concept, description: "Placed by a customer" } });
});

test("toggling the abstract checkbox calls onUpdate", async () => {
  const onUpdate = vi.fn();
  render(ObjectInspector, { props: { node, onUpdate, profileName: "uml-domain" } });
  const checkbox = screen.getByRole("checkbox") as HTMLInputElement;
  await fireEvent.click(checkbox);
  expect(onUpdate).toHaveBeenCalledWith({ abstract: true });
});

describe("ObjectInspector associations", () => {
  const other: ModelNode = {
    concept: { id: "n2", type: "uml.Class", title: "Customer", body: "" },
    key: "n2", type: "uml.Class", stereotypes: [], attributes: [], position: { x: 0, y: 0 },
  };
  const edge: ModelEdge = {
    id: "e1", kind: "associates", from: "n1", to: "n2",
    fromEnd: {}, toEnd: { multiplicity: "0..*" }, bidirectional: false,
  };

  it("lists the related node before the attributes section", () => {
    render(ObjectInspector, { props: { node, onUpdate: () => {}, profileName: "uml-domain", nodes: [node, other], edges: [edge] } });
    expect(screen.getByText("Associations")).toBeTruthy();
    expect(screen.getByText("Customer")).toBeTruthy();
    const doc = document.body.textContent ?? "";
    expect(doc.indexOf("Associations")).toBeLessThan(doc.indexOf("Attributes"));
  });

  it("shows an empty hint when the node has no relationships", () => {
    render(ObjectInspector, { props: { node, onUpdate: () => {}, profileName: "uml-domain", nodes: [node], edges: [] } });
    expect(screen.getByText("No associations")).toBeTruthy();
  });
});

describe("ObjectInspector palette", () => {
  it("offers the profile's metaclasses in the type datalist", () => {
    const { container } = render(ObjectInspector, { props: { node, onUpdate: () => {}, profileName: "uml-domain" } });
    const options = [...container.querySelectorAll("datalist#okf-metaclasses option")].map(o => o.getAttribute("value"));
    expect(options).toEqual(["uml.Class", "uml.Interface", "uml.Enum", "uml.DataType", "uml.Actor", "uml.UseCase"]);
  });

  it("offers the profile's stereotypes in a datalist", () => {
    const { container } = render(ObjectInspector, { props: { node, onUpdate: () => {}, profileName: "uml-domain" } });
    const options = [...container.querySelectorAll("datalist#okf-stereotypes option")].map(o => o.getAttribute("value"));
    expect(options).toContain("aggregateRoot");
  });

  it("switching type to uml.Enum shows the values editor", () => {
    const onUpdate = vi.fn();
    render(ObjectInspector, { props: { node: { ...node, type: "uml.Enum", values: ["A"] }, onUpdate, profileName: "uml-domain" } });
    expect(screen.getByText("Values (one per line)")).toBeTruthy();
  });
});
