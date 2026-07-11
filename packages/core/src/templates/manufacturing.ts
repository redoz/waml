import type { ModelGraph } from "@uaml/okf";
import { f, mart, rel, type Template } from "./helpers";

// Manufacturing / Supply Chain — shop-floor + inbound model. fct_production_runs
// carries the OEE decomposition per run; downtime events explain the
// availability leg; quality inspections the quality leg; purchase orders give
// supplier OTIF and inbound defects.
//
// Goal coverage (niche "manufacturing"):
//   OEE per line            → fct_production_runs (availability × performance × quality)
//   unplanned downtime      → fct_downtime_events (is_planned=false) × dim_machine.is_bottleneck
//   defect & scrap rate     → fct_quality_inspections (defects_found, scrap_units) by product/shift
//   supplier OTIF           → fct_purchase_orders (is_on_time, is_in_full, defect_ppm)
//   plan attainment         → fct_production_runs (planned_units vs produced_units) by shift
const graph: ModelGraph = {
  diagrams: [],
  nodes: [
    mart("dim_plant", "Plant", "TABLE", [
      f("plant_id", "STRING", true, "Unique plant identifier."),
      f("name", "STRING", false, "Plant name."),
      f("country", "STRING", false, "Country the plant operates in."),
      f("plant_type", "STRING", false, "What the plant does: assembly, machining, packaging."),
      f("shifts_per_day", "INTEGER", false, "Shift pattern the plant runs."),
    ], "One row per manufacturing plant."),
    mart("dim_machine", "Machine", "TABLE", [
      f("machine_id", "STRING", true, "Unique machine identifier."),
      f("plant_id", "STRING", false, "Plant the machine is installed in."),
      f("line", "STRING", false, "Production line the machine belongs to."),
      f("machine_type", "STRING", false, "Machine class (press, CNC, filler, robot cell)."),
      f("commissioned_at", "DATE", false, "When the machine entered service."),
      f("is_bottleneck", "BOOLEAN", false, "Whether the machine constrains line throughput."),
    ], "One row per machine with its line and bottleneck flag."),
    mart("dim_product", "Product", "TABLE", [
      f("product_id", "STRING", true, "Unique product identifier."),
      f("sku", "STRING", false, "Finished-good SKU code."),
      f("name", "STRING", false, "Product name."),
      f("family", "STRING", false, "Product family for rollups."),
      f("standard_cycle_secs", "INTEGER", false, "Ideal cycle time — the performance-leg baseline."),
      f("standard_cost", "NUMERIC", false, "Standard cost per unit — scrap valuation."),
    ], "One row per manufactured product with standard cycle and cost."),
    mart("dim_supplier", "Supplier", "TABLE", [
      f("supplier_id", "STRING", true, "Unique supplier identifier."),
      f("name", "STRING", false, "Supplier name."),
      f("country", "STRING", false, "Supplier's country."),
      f("category", "STRING", false, "What the supplier provides (raw material, components, packaging)."),
      f("is_strategic", "BOOLEAN", false, "Whether the supplier is on the strategic list."),
    ], "Reference of inbound suppliers."),
    mart("fct_production_runs", "Production Runs", "VIEW", [
      f("run_id", "STRING", true, "Unique production-run identifier."),
      f("machine_id", "STRING", false, "Machine that executed the run."),
      f("product_id", "STRING", false, "Product manufactured."),
      f("started_at", "TIMESTAMP", false, "Run start."),
      f("ended_at", "TIMESTAMP", false, "Run end."),
      f("shift", "STRING", false, "Shift the run belongs to — plan attainment by shift."),
      f("planned_units", "INTEGER", false, "Units planned for the run."),
      f("produced_units", "INTEGER", false, "Units actually produced."),
      f("good_units", "INTEGER", false, "Units that passed quality first time."),
      f("availability_pct", "FLOAT", false, "Run time ÷ planned time — the availability leg of OEE."),
      f("performance_pct", "FLOAT", false, "Actual speed ÷ ideal cycle — the performance leg."),
      f("quality_pct", "FLOAT", false, "good_units ÷ produced_units — the quality leg."),
      f("oee_pct", "FLOAT", false, "availability × performance × quality."),
    ], "One row per production run with the full OEE decomposition."),
    mart("fct_downtime_events", "Downtime Events", "VIEW", [
      f("downtime_id", "STRING", true, "Unique downtime-event identifier."),
      f("machine_id", "STRING", false, "Machine that stopped."),
      f("started_at", "TIMESTAMP", false, "When the stop began."),
      f("duration_mins", "INTEGER", false, "How long the machine was down."),
      f("is_planned", "BOOLEAN", false, "Planned maintenance vs unplanned stop."),
      f("reason_code", "STRING", false, "Standardized stop-reason code."),
      f("category", "STRING", false, "Stop class: breakdown, changeover, material wait, maintenance."),
    ], "One row per machine stop — the availability-loss ledger."),
    mart("fct_quality_inspections", "Quality Inspections", "VIEW", [
      f("inspection_id", "STRING", true, "Unique inspection identifier."),
      f("run_id", "STRING", false, "Production run inspected."),
      f("inspected_at", "TIMESTAMP", false, "When the inspection happened."),
      f("sample_size", "INTEGER", false, "Units inspected."),
      f("defects_found", "INTEGER", false, "Defective units in the sample — defect rate."),
      f("defect_type", "STRING", false, "Dominant defect classification."),
      f("scrap_units", "INTEGER", false, "Units scrapped as a result."),
      f("scrap_cost", "NUMERIC", false, "Scrap valued at standard cost."),
    ], "One row per quality inspection. Defects and scrap cost."),
    mart("fct_purchase_orders", "Purchase Orders", "VIEW", [
      f("po_id", "STRING", true, "Unique purchase-order identifier."),
      f("supplier_id", "STRING", false, "Supplier the order was placed with."),
      f("product_id", "STRING", false, "Material or component ordered."),
      f("ordered_at", "DATE", false, "Order date."),
      f("promised_date", "DATE", false, "Delivery date the supplier committed to."),
      f("received_date", "DATE", false, "Date the goods arrived."),
      f("quantity_ordered", "INTEGER", false, "Units ordered."),
      f("quantity_received", "INTEGER", false, "Units received."),
      f("is_on_time", "BOOLEAN", false, "Arrived by the promised date — the OT of OTIF."),
      f("is_in_full", "BOOLEAN", false, "Full quantity delivered — the IF of OTIF."),
      f("defect_ppm", "INTEGER", false, "Inbound defects per million — supplier quality."),
    ], "One row per purchase order. Supplier OTIF and inbound quality."),
  ],
  edges: [
    rel("e1", "dim_machine", "dim_plant", "plant_id", "plant_id"),
    rel("e2", "fct_production_runs", "dim_machine", "machine_id", "machine_id"),
    rel("e3", "fct_production_runs", "dim_product", "product_id", "product_id"),
    rel("e4", "fct_downtime_events", "dim_machine", "machine_id", "machine_id"),
    rel("e5", "fct_quality_inspections", "fct_production_runs", "run_id", "run_id"),
    rel("e6", "fct_purchase_orders", "dim_supplier", "supplier_id", "supplier_id"),
    rel("e7", "fct_purchase_orders", "dim_product", "product_id", "product_id"),
  ],
};

export const manufacturing: Template = {
  id: "manufacturing",
  nicheId: "manufacturing",
  category: "industry",
  name: "Manufacturing / Supply Chain",
  description: "Shop floor & inbound: plants, machines, products, suppliers, production runs (OEE), downtime, quality inspections and purchase orders (OTIF).",
  graph,
};
