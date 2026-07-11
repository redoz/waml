import type { ModelGraph } from "@uaml/okf";
import { f, mart, rel, type Template } from "./helpers";

// Retail Chain / POS & Inventory — the Kimball classic. fct_pos_sales is the
// receipt-line stream; fct_inventory_daily the stock position (stockouts,
// weeks of supply); replenishment closes the supply loop; promotions and
// shrinkage explain margin; store traffic gives conversion.
//
// Goal coverage (niche "retail_pos"):
//   same-store & basket     → fct_store_traffic (transactions, avg_basket) + fct_pos_sales
//   stockouts on A-SKUs     → fct_inventory_daily (is_stockout) × dim_product.velocity_band
//   turns & weeks-of-supply → fct_inventory_daily.weeks_of_supply × replenishment lead times
//   promo uplift            → fct_pos_sales (promotion_id, discount) vs non-promo baseline
//   shrinkage hot spots     → fct_shrinkage_events (reason, cost) by store × category
const graph: ModelGraph = {
  diagrams: [],
  nodes: [
    mart("dim_store", "Store", "TABLE", [
      f("store_id", "STRING", true, "Unique store identifier."),
      f("name", "STRING", false, "Store name or number."),
      f("city", "STRING", false, "City the store is located in."),
      f("format", "STRING", false, "Store format: hypermarket, supermarket, convenience."),
      f("opened_at", "DATE", false, "Opening date — same-store comparisons need ≥13 months."),
      f("selling_area_m2", "INTEGER", false, "Selling area — sales-density denominator."),
    ], "One row per store."),
    mart("dim_product", "Product", "TABLE", [
      f("product_id", "STRING", true, "Unique product identifier."),
      f("sku", "STRING", false, "Stock-keeping unit code."),
      f("name", "STRING", false, "Product display name."),
      f("category", "STRING", false, "Top-level category."),
      f("subcategory", "STRING", false, "Second-level grouping."),
      f("brand", "STRING", false, "Brand label."),
      f("unit_cost", "NUMERIC", false, "Landed unit cost — margin and shrink valuation."),
      f("list_price", "NUMERIC", false, "Regular shelf price."),
      f("velocity_band", "STRING", false, "A/B/C velocity class — stockouts on A hurt most."),
    ], "One row per SKU with cost and velocity class."),
    mart("dim_promotion", "Promotion", "TABLE", [
      f("promotion_id", "STRING", true, "Unique promotion identifier."),
      f("name", "STRING", false, "Promotion name."),
      f("promo_type", "STRING", false, "Mechanic: discount, BOGO, loyalty points."),
      f("start_date", "DATE", false, "Promotion start."),
      f("end_date", "DATE", false, "Promotion end."),
      f("discount_pct", "FLOAT", false, "Depth of the discount — uplift-vs-margin trade-off."),
    ], "Reference of promotions and their mechanics."),
    mart("fct_pos_sales", "POS Sales", "CONNECTOR", [
      f("sale_id", "STRING", true, "Unique identifier for the receipt line."),
      f("store_id", "STRING", false, "Store that rang the sale."),
      f("product_id", "STRING", false, "SKU sold."),
      f("promotion_id", "STRING", false, "Promotion applied, null when sold at regular price."),
      f("sold_at", "TIMESTAMP", false, "When the sale was rung."),
      f("basket_id", "STRING", false, "Receipt the line belongs to — basket analysis."),
      f("quantity", "INTEGER", false, "Units sold on the line."),
      f("unit_price", "NUMERIC", false, "Price charged per unit."),
      f("discount", "NUMERIC", false, "Discount applied to the line."),
    ], "One row per receipt line — the POS stream."),
    mart("fct_inventory_daily", "Inventory (daily)", "VIEW", [
      f("snapshot_id", "STRING", true, "Unique identifier for the store-SKU-day snapshot."),
      f("store_id", "STRING", false, "Store holding the stock."),
      f("product_id", "STRING", false, "SKU counted."),
      f("snapshot_date", "DATE", false, "Day of the snapshot."),
      f("on_hand_units", "INTEGER", false, "Units on hand at day end."),
      f("on_order_units", "INTEGER", false, "Units on open purchase orders."),
      f("is_stockout", "BOOLEAN", false, "Whether the SKU was out of stock — availability loss."),
      f("weeks_of_supply", "FLOAT", false, "on_hand ÷ weekly run-rate — slow-mover detector."),
    ], "One row per store × SKU × day of stock position."),
    mart("fct_replenishment", "Replenishment", "VIEW", [
      f("order_id", "STRING", true, "Unique replenishment-order identifier."),
      f("store_id", "STRING", false, "Store being replenished."),
      f("product_id", "STRING", false, "SKU ordered."),
      f("ordered_at", "DATE", false, "When the order was placed."),
      f("received_at", "DATE", false, "When the goods arrived."),
      f("quantity_ordered", "INTEGER", false, "Units ordered."),
      f("quantity_received", "INTEGER", false, "Units actually received."),
      f("lead_time_days", "INTEGER", false, "Order-to-receipt lead time."),
      f("fill_rate_pct", "FLOAT", false, "received ÷ ordered — supplier service level."),
    ], "One row per replenishment order. Lead times and fill rate."),
    mart("fct_shrinkage_events", "Shrinkage", "VIEW", [
      f("shrink_id", "STRING", true, "Unique shrinkage-event identifier."),
      f("store_id", "STRING", false, "Store where the loss was recorded."),
      f("product_id", "STRING", false, "SKU lost."),
      f("recorded_at", "DATE", false, "Day the loss was recorded."),
      f("reason", "STRING", false, "Loss reason: theft, damage, expiry, admin error."),
      f("units_lost", "INTEGER", false, "Units written off."),
      f("shrink_cost", "NUMERIC", false, "Cost of the loss at unit cost."),
    ], "One row per shrinkage write-off — hot spots by store and category."),
    mart("fct_store_traffic", "Store Traffic", "VIEW", [
      f("traffic_id", "STRING", true, "Unique identifier for the store-day traffic record."),
      f("store_id", "STRING", false, "Store measured."),
      f("traffic_date", "DATE", false, "Calendar day."),
      f("footfall", "INTEGER", false, "Visitors counted entering the store."),
      f("transactions", "INTEGER", false, "Receipts closed — conversion numerator."),
      f("conversion_pct", "FLOAT", false, "transactions ÷ footfall."),
      f("avg_basket", "NUMERIC", false, "Average receipt value — basket size."),
    ], "One row per store × day. Footfall, conversion and basket size."),
  ],
  edges: [
    rel("e1", "fct_pos_sales", "dim_store", "store_id", "store_id"),
    rel("e2", "fct_pos_sales", "dim_product", "product_id", "product_id"),
    rel("e3", "fct_pos_sales", "dim_promotion", "promotion_id", "promotion_id"),
    rel("e4", "fct_inventory_daily", "dim_store", "store_id", "store_id"),
    rel("e5", "fct_inventory_daily", "dim_product", "product_id", "product_id"),
    rel("e6", "fct_replenishment", "dim_store", "store_id", "store_id"),
    rel("e7", "fct_replenishment", "dim_product", "product_id", "product_id"),
    rel("e8", "fct_shrinkage_events", "dim_store", "store_id", "store_id"),
    rel("e9", "fct_shrinkage_events", "dim_product", "product_id", "product_id"),
    rel("e10", "fct_store_traffic", "dim_store", "store_id", "store_id"),
  ],
};

export const retail_pos: Template = {
  id: "retail_pos",
  nicheId: "retail_pos",
  category: "industry",
  name: "Retail Chain / POS & Inventory",
  description: "Brick-and-mortar retail: stores, SKUs with velocity bands, promotions, POS lines, daily inventory (stockouts), replenishment, shrinkage and traffic.",
  graph,
};
