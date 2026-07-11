import type { ModelGraph } from "@mc/okf";
import { f, mart, rel, type Template } from "./helpers";

// Telecom / ISP — subscriber-lifecycle model. fct_usage_daily is the network
// engagement stream; fct_billing carries collections and involuntary
// disconnects; fct_plan_changes the upgrade/migration waterfall; network
// incidents by region tie service quality to churn.
//
// Goal coverage (niche "telecom"):
//   churn in HV segments    → dim_subscriber (churned_at, segment) × fct_billing.amount
//   ARPU growth             → fct_billing (amount, addons_amount) ÷ active subscribers
//   QoS-driven churn        → fct_network_incidents × dim_region × churned subscribers
//   collections/involuntary → fct_billing (is_late, dunning_stage)
//   5G/fiber migration      → fct_plan_changes (change_type='migration') × dim_plan.network_gen
const graph: ModelGraph = {
  diagrams: [],
  nodes: [
    mart("dim_subscriber", "Subscriber", "VIEW", [
      f("subscriber_id", "STRING", true, "Unique subscriber identifier."),
      f("activated_at", "DATE", false, "Service activation date."),
      f("plan_id", "STRING", false, "Plan the subscriber is currently on."),
      f("region_id", "STRING", false, "Region the subscriber lives in."),
      f("segment", "STRING", false, "Value segment: consumer, premium, business."),
      f("tenure_months", "INTEGER", false, "Months since activation."),
      f("is_active", "BOOLEAN", false, "Whether the line is currently active."),
      f("churned_at", "DATE", false, "Disconnect date, if churned."),
      f("churn_reason", "STRING", false, "Recorded churn reason (price, coverage, competitor)."),
    ], "One row per subscriber line with plan, segment and churn state."),
    mart("dim_plan", "Plan", "TABLE", [
      f("plan_id", "STRING", true, "Unique plan identifier."),
      f("name", "STRING", false, "Commercial plan name."),
      f("network_gen", "STRING", false, "Network generation: 4G, 5G, fiber — the migration axis."),
      f("monthly_fee", "NUMERIC", false, "Monthly recurring fee — the ARPU base."),
      f("data_gb", "INTEGER", false, "Monthly data allowance in GB."),
      f("is_unlimited", "BOOLEAN", false, "Whether the plan has unlimited data."),
    ], "Reference of tariff plans by network generation."),
    mart("dim_region", "Region", "TABLE", [
      f("region_id", "STRING", true, "Unique region identifier."),
      f("name", "STRING", false, "Region name."),
      f("market_type", "STRING", false, "urban / suburban / rural — coverage economics."),
      f("population_covered", "INTEGER", false, "Population within network coverage."),
    ], "Reference of service regions."),
    mart("fct_usage_daily", "Usage (daily)", "CONNECTOR", [
      f("usage_id", "STRING", true, "Unique identifier for the subscriber-day usage record."),
      f("subscriber_id", "STRING", false, "Subscriber who generated the usage."),
      f("usage_date", "DATE", false, "Calendar day of usage."),
      f("data_mb", "INTEGER", false, "Data consumed in megabytes."),
      f("voice_mins", "INTEGER", false, "Voice minutes used."),
      f("sms_count", "INTEGER", false, "SMS messages sent."),
      f("roaming_mb", "INTEGER", false, "Roaming data consumed — overage revenue signal."),
    ], "One row per subscriber × day of network usage."),
    mart("fct_billing", "Billing", "VIEW", [
      f("invoice_id", "STRING", true, "Unique invoice identifier."),
      f("subscriber_id", "STRING", false, "Subscriber billed."),
      f("billed_at", "DATE", false, "Invoice date."),
      f("amount", "NUMERIC", false, "Total invoiced amount — the ARPU numerator."),
      f("addons_amount", "NUMERIC", false, "Add-on charges within the invoice — attach revenue."),
      f("status", "STRING", false, "Payment status of the invoice."),
      f("paid_at", "DATE", false, "Date the invoice was paid."),
      f("is_late", "BOOLEAN", false, "Whether payment missed the due date — collections signal."),
      f("dunning_stage", "STRING", false, "Current dunning stage — the involuntary-disconnect pipeline."),
    ], "One row per invoice. ARPU, collections and dunning."),
    mart("fct_plan_changes", "Plan Changes", "VIEW", [
      f("change_id", "STRING", true, "Unique identifier for the plan change."),
      f("subscriber_id", "STRING", false, "Subscriber who changed plan."),
      f("changed_at", "DATE", false, "When the change took effect."),
      f("plan_from", "STRING", false, "Plan before the change."),
      f("plan_to", "STRING", false, "Plan after the change."),
      f("change_type", "STRING", false, "upgrade / downgrade / migration (e.g. 4G→5G, copper→fiber)."),
      f("mrr_delta", "NUMERIC", false, "Signed change in monthly recurring revenue."),
    ], "One row per plan change. Upgrades and network-generation migration."),
    mart("fct_addon_orders", "Add-on Orders", "VIEW", [
      f("order_id", "STRING", true, "Unique add-on order identifier."),
      f("subscriber_id", "STRING", false, "Subscriber who ordered the add-on."),
      f("ordered_at", "DATE", false, "When the add-on was ordered."),
      f("addon_type", "STRING", false, "Kind of add-on: device, TV, insurance, roaming pack."),
      f("monthly_fee", "NUMERIC", false, "Recurring fee added by the add-on."),
      f("one_off_price", "NUMERIC", false, "One-time price paid upfront."),
    ], "One row per add-on order. The attach-rate lever behind ARPU."),
    mart("fct_network_incidents", "Network Incidents", "VIEW", [
      f("incident_id", "STRING", true, "Unique incident identifier."),
      f("region_id", "STRING", false, "Region hit by the incident."),
      f("started_at", "TIMESTAMP", false, "When the outage or degradation began."),
      f("resolved_at", "TIMESTAMP", false, "When service was restored."),
      f("severity", "STRING", false, "Incident severity class."),
      f("affected_subscribers", "INTEGER", false, "Subscribers impacted — the QoS-churn link."),
      f("service_type", "STRING", false, "Service affected: mobile data, voice, broadband."),
    ], "One row per network incident. Service quality by region."),
  ],
  edges: [
    rel("e1", "dim_subscriber", "dim_plan", "plan_id", "plan_id"),
    rel("e2", "dim_subscriber", "dim_region", "region_id", "region_id"),
    rel("e3", "fct_usage_daily", "dim_subscriber", "subscriber_id", "subscriber_id"),
    rel("e4", "fct_billing", "dim_subscriber", "subscriber_id", "subscriber_id"),
    rel("e5", "fct_plan_changes", "dim_subscriber", "subscriber_id", "subscriber_id"),
    rel("e6", "fct_addon_orders", "dim_subscriber", "subscriber_id", "subscriber_id"),
    rel("e7", "fct_network_incidents", "dim_region", "region_id", "region_id"),
  ],
};

export const telecom: Template = {
  id: "telecom",
  nicheId: "telecom",
  category: "industry",
  name: "Telecom / ISP",
  description: "Subscriber lifecycle: plans by network generation, regions, daily usage, billing & dunning, plan migrations, add-ons and network incidents.",
  graph,
};
