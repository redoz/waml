import { describe, it, expect, beforeAll } from "vitest";
import { initWasm, new_diagram_doc } from "./index";

describe("new_diagram_doc", () => {
  beforeAll(async () => {
    await initWasm();
  });

  it("emits a uml.Activity doc for the activity kind", () => {
    const md = new_diagram_doc("activity", "Checkout");
    expect(md).toContain('type: "uml.Activity"');
    expect(md).toContain('title: "Checkout"');
  });

  it("emits a Diagram + uml-domain doc for the class kind", () => {
    const md = new_diagram_doc("class", "My Domain");
    expect(md).toContain('type: "Diagram"');
    expect(md).toContain('profile: "uml-domain"');
  });
});
