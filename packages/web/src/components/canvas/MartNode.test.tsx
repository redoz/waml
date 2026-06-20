import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { ReactFlowProvider } from "@xyflow/react";
import { MartNode } from "./MartNode";

const node = {
  key: "n1", title: "Users", inputSource: "VIEW", status: "created", owoxId: "x",
  position: { x: 0, y: 0 },
  schema: [
    { name: "id", type: "INT64", pk: true },
    { name: "email", type: "STRING", pk: false },
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

describe("MartNode ERD rendering", () => {
  it("shows the field count (not rows) in compact mode", () => {
    renderNode("compact");
    expect(screen.getByText("2 fields")).toBeTruthy();
    expect(screen.queryByText("INT64")).toBeNull();
  });

  it("shows each field name and type in ERD mode", () => {
    renderNode("erd");
    expect(screen.getByText("id")).toBeTruthy();
    expect(screen.getByText("INT64")).toBeTruthy();
    expect(screen.getByText("email")).toBeTruthy();
    expect(screen.getByText("STRING")).toBeTruthy();
  });
});
