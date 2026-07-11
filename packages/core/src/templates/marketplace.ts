import type { ModelGraph } from "@uaml/okf";
import { f, mart, rel, type Template } from "./helpers";

// Marketplace / Platform — two-sided model. Supply (sellers, listings) and
// demand (buyers, search) are deliberately separate branches; fct_orders is the
// match where they meet, carrying GMV, take rate and fill. Liquidity = the rate
// at which search requests and listings convert into completed orders.
//
// Goal coverage (niche "marketplace"):
//   liquidity               → fct_search_requests (converted, time_to_match) × fct_orders
//   supply utilization      → fct_listings (is_available) × dim_seller (is_activated)
//   take-rate optimisation  → dim_category.take_rate_pct × fct_orders (gmv, take_rate)
//   fill failures           → fct_cancellations (stage, cancelled_by) + fct_orders.fulfillment_mins
//   repeat both sides       → dim_buyer.is_repeat + dim_seller × fct_orders cohorts
const graph: ModelGraph = {
  diagrams: [],
  nodes: [
    mart("dim_buyer", "Buyer", "VIEW", [
      f("buyer_id", "STRING", true, "Unique buyer identifier."),
      f("signup_date", "DATE", false, "When the buyer first registered."),
      f("acquisition_channel", "STRING", false, "Marketing source that brought the buyer in."),
      f("region", "STRING", false, "Buyer's geographic region."),
      f("segment", "STRING", false, "Buyer segment for targeting and analysis."),
      f("lifetime_orders", "INTEGER", false, "Total orders placed to date."),
      f("is_repeat", "BOOLEAN", false, "Made 2+ orders — demand-side retention."),
    ], "Demand side: one row per buyer."),
    mart("dim_seller", "Seller", "VIEW", [
      f("seller_id", "STRING", true, "Unique seller identifier."),
      f("onboarded_at", "DATE", false, "When the seller joined the platform."),
      f("category", "STRING", false, "Primary category the seller sells in."),
      f("region", "STRING", false, "Seller's geographic region."),
      f("rating", "FLOAT", false, "Average buyer rating of the seller."),
      f("active_listings", "INTEGER", false, "Number of currently live listings."),
      f("is_activated", "BOOLEAN", false, "Reached first sale — supply activation."),
      f("fulfillment_type", "STRING", false, "How the seller fulfils orders."),
    ], "Supply side: one row per seller/supplier."),
    mart("dim_category", "Category", "TABLE", [
      f("category_id", "STRING", true, "Unique category identifier."),
      f("name", "STRING", false, "Display name of the category."),
      f("parent_category", "STRING", false, "Parent grouping in the category tree."),
      f("take_rate_pct", "FLOAT", false, "Platform's standard cut for the category — the take-rate optimisation lever."),
    ], "Reference of listing categories with their standard take rates."),
    mart("fct_listings", "Listings", "VIEW", [
      f("listing_id", "STRING", true, "Unique listing identifier."),
      f("seller_id", "STRING", false, "Seller that owns the listing."),
      f("created_at", "TIMESTAMP", false, "When the listing was created."),
      f("category_id", "STRING", false, "Category the listing belongs to."),
      f("price", "NUMERIC", false, "Listed price of the offer."),
      f("status", "STRING", false, "Current listing status."),
      f("is_available", "BOOLEAN", false, "Live inventory — supply availability."),
    ], "One row per listing/offer. Supply inventory and availability."),
    mart("fct_search_requests", "Search Requests", "CONNECTOR", [
      f("request_id", "STRING", true, "Unique search request identifier."),
      f("buyer_id", "STRING", false, "Buyer who made the search."),
      f("requested_at", "TIMESTAMP", false, "When the search was made."),
      f("query", "STRING", false, "Raw search text entered by the buyer."),
      f("category", "STRING", false, "Category the search was scoped to."),
      f("results_count", "INTEGER", false, "Number of results returned."),
      f("clicked", "BOOLEAN", false, "Whether the buyer clicked a result."),
      f("converted", "BOOLEAN", false, "Whether the search led to an order."),
      f("time_to_match_mins", "FLOAT", false, "Search → transaction latency."),
    ], "One row per search/browse request. Demand and match-quality signal."),
    mart("fct_orders", "Orders", "VIEW", [
      f("order_id", "STRING", true, "Unique order identifier."),
      f("buyer_id", "STRING", false, "Buyer on the order."),
      f("seller_id", "STRING", false, "Seller on the order."),
      f("listing_id", "STRING", false, "Listing that was purchased."),
      f("ordered_at", "TIMESTAMP", false, "When the order was placed."),
      f("gmv", "NUMERIC", false, "Gross merchandise value."),
      f("take_rate", "FLOAT", false, "Platform's cut as a fraction of GMV."),
      f("platform_revenue", "NUMERIC", false, "gmv × take_rate."),
      f("status", "STRING", false, "Current order status."),
      f("is_fulfilled", "BOOLEAN", false, "Whether the order was fulfilled."),
      f("fulfillment_mins", "FLOAT", false, "Order-to-fulfilment time — fill speed."),
    ], "The match: one row per completed transaction. GMV, take rate and fill."),
    mart("fct_reviews", "Reviews", "VIEW", [
      f("review_id", "STRING", true, "Unique review identifier."),
      f("order_id", "STRING", false, "Order the review relates to."),
      f("rating", "INTEGER", false, "Buyer's star rating for the order."),
      f("created_at", "TIMESTAMP", false, "When the review was submitted."),
      f("has_complaint", "BOOLEAN", false, "Whether the review flags a complaint."),
    ], "One row per post-transaction review. Trust and retention signal."),
    mart("fct_cancellations", "Cancellations", "VIEW", [
      f("cancellation_id", "STRING", true, "Unique cancellation identifier."),
      f("order_id", "STRING", false, "Order that was cancelled."),
      f("cancelled_at", "TIMESTAMP", false, "When the cancellation happened."),
      f("cancelled_by", "STRING", false, "Who cancelled: buyer, seller or platform."),
      f("stage", "STRING", false, "Order stage at cancellation (pre-payment, pre-fulfilment, in-transit)."),
      f("reason", "STRING", false, "Stated cancellation reason."),
      f("refund_amount", "NUMERIC", false, "Amount refunded to the buyer."),
    ], "One row per cancelled order — fill-rate failures and their causes."),
  ],
  edges: [
    rel("e1", "fct_listings", "dim_seller", "seller_id", "seller_id"),
    rel("e2", "fct_search_requests", "dim_buyer", "buyer_id", "buyer_id"),
    rel("e3", "fct_orders", "dim_buyer", "buyer_id", "buyer_id"),
    rel("e4", "fct_orders", "dim_seller", "seller_id", "seller_id"),
    rel("e5", "fct_orders", "fct_listings", "listing_id", "listing_id"),
    rel("e6", "fct_reviews", "fct_orders", "order_id", "order_id"),
    rel("e7", "fct_listings", "dim_category", "category_id", "category_id"),
    rel("e8", "fct_cancellations", "fct_orders", "order_id", "order_id"),
  ],
};

export const marketplace: Template = {
  id: "marketplace",
  nicheId: "marketplace",
  category: "industry",
  name: "Marketplace",
  description: "Two-sided platform: buyers, sellers, listings & categories, search demand, GMV/take-rate orders, cancellations and reviews.",
  graph,
};
