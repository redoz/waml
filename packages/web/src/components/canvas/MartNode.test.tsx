import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { ReactFlowProvider } from "@xyflow/react";
import { MartNode } from "./MartNode";

const node = {
  key: "n1", title: "Users", type: "uml.Class", stereotypes: [],
  position: { x: 0, y: 0 },
  attributes: [
    { name: "id", type: { name: "INT64" }, multiplicity: "1" },
    { name: "email", type: { name: "STRING" }, multiplicity: "1" },
  ],
};

function renderNode(viewMode: "compact" | "erd") {
  // MartNode is a React Flow node component; Handle needs the RF provider context.
  return render(
    <ReactFlowProvider>
      {/* @ts-expect-error minimal NodeProps for a render-only test */}
      <MartNode id="n1" data={{ ...node, _viewMode: viewMode }} />
    </ReactFlowProvider>,
  );
}

describe("MartNode rendering", () => {
  it("shows the title and field count (not rows) in compact mode", () => {
    const { container } = renderNode("compact");
    expect(screen.getByText("Users")).toBeTruthy();
    expect(screen.getByText("2 fields")).toBeTruthy();
    expect(screen.queryByText("INT64")).toBeNull();
    // No status dot markup (status is a data-profile concern, dropped).
    expect(container.querySelector(".animate-pulse")).toBeNull();
  });

  it("shows each attribute name and type token in ERD mode", () => {
    renderNode("erd");
    expect(screen.getByText("id")).toBeTruthy();
    expect(screen.getByText("INT64")).toBeTruthy();
    expect(screen.getByText("email")).toBeTruthy();
    expect(screen.getByText("STRING")).toBeTruthy();
  });
});
