import { describe, it, expect } from "vitest";
import { TEMPLATES, INDUSTRY_TEMPLATES, DATASET_TEMPLATES } from "../src/templates";
import { serializeBundle, parseBundle } from "@mc/okf";

// Grows as template tasks land; the assertion pins the full expected library.
const EXPECTED_INDUSTRY_IDS = [
  "ecommerce", "saas", "marketplace", "marketing_ads", "mobile_gaming", "finance", "medical",
  "ott_media", "delivery_logistics", "insurance", "b2b_sales", "customer_support",
  "hr_people", "telecom", "hospitality", "restaurants", "edtech", "travel_ota",
  "retail_pos", "manufacturing",
];
const EXPECTED_DATASET_IDS = ["crypto_bitcoin", "stackoverflow"];

it("ships the expected library", () => {
  expect(INDUSTRY_TEMPLATES.map(t => t.id).sort()).toEqual([...EXPECTED_INDUSTRY_IDS].sort());
  expect(DATASET_TEMPLATES.map(t => t.id).sort()).toEqual([...EXPECTED_DATASET_IDS].sort());
});

for (const t of TEMPLATES) {
  describe(t.name, () => {
    const byKey = new Map(t.graph.nodes.map(n => [n.key, n]));

    it("has unique node keys and edge ids", () => {
      expect(byKey.size).toBe(t.graph.nodes.length);
      expect(new Set(t.graph.edges.map(e => e.id)).size).toBe(t.graph.edges.length);
    });

    it("every node has fully described attributes", () => {
      for (const n of t.graph.nodes) {
        expect(n.attributes.length, `${n.title} has attributes`).toBeGreaterThan(0);
        expect(n.description?.trim(), `${n.title} has a description`).toBeTruthy();
        for (const a of n.attributes) {
          expect(a.description?.trim(), `${n.title}.${a.name} is described`).toBeTruthy();
        }
      }
    });

    it("every edge resolves and is an associates relationship", () => {
      for (const e of t.graph.edges) {
        const from = byKey.get(e.from);
        const to = byKey.get(e.to);
        expect(from, `${e.id} from ${e.from}`).toBeTruthy();
        expect(to, `${e.id} to ${e.to}`).toBeTruthy();
        expect(e.kind, `${e.id} kind`).toBe("associates");
        expect(e.fromEnd, `${e.id} has a fromEnd`).toBeTruthy();
        expect(e.toEnd, `${e.id} has a toEnd`).toBeTruthy();
      }
    });

    it("round-trips through OKF", () => {
      const g = parseBundle(serializeBundle(t.graph, t.name).files);
      expect(g.nodes.length).toBe(t.graph.nodes.length);
      expect(g.edges.length).toBe(t.graph.edges.length);
    });
  });
}

// Money must be NUMERIC in industry templates (BigQuery convention); FLOAT is
// reserved for true rates/scores. Datasets stay schema-faithful to the source.
const MONEY = /revenue|cost|price|amount|gmv|mrr|spend|fee|salary|premium|payout|margin|_value|ltv|balance|payment/i;
const EXEMPT = /pct|rate|score|ratio|band|count|qty|quantity|_id$|days|mins|secs|hours|multiplier/i;

for (const t of INDUSTRY_TEMPLATES) {
  it(`${t.id}: money fields are NUMERIC`, () => {
    for (const n of t.graph.nodes) {
      for (const f of n.attributes) {
        if (f.type.name === "FLOAT" && MONEY.test(f.name) && !EXEMPT.test(f.name)) {
          throw new Error(`${t.id}.${n.key}.${f.name} is FLOAT — money must be NUMERIC`);
        }
      }
    }
  });
}
