import type { ModelGraph } from "@uaml/okf";
import { f, mart, rel, type Template } from "./helpers";

// Insurance (P&C) — underwriting + claims model. The funnel runs fct_quotes
// (quote-to-bind) → fct_policies (renewals) → fct_premiums_monthly (earned
// premium & expenses); fct_claims → fct_claim_payments carry incurred losses.
// Loss ratio = incurred losses ÷ earned premium; combined ratio adds expenses.
//
// Goal coverage (niche "insurance"):
//   loss ratio by segment   → fct_claim_payments ÷ fct_premiums_monthly × dim_policyholder.risk_segment
//   combined ratio          → + fct_premiums_monthly (commission_expense, admin_expense)
//   renewal retention       → fct_policies (is_renewal, expiry_date) cohorts
//   quote-to-bind           → fct_quotes (is_bound, premium_quoted) × dim_agent.channel
//   claims cycle & leakage  → fct_claims.cycle_days + fct_claim_payments.leakage_amount
const graph: ModelGraph = {
  diagrams: [],
  nodes: [
    mart("dim_policyholder", "Policyholder", "VIEW", [
      f("policyholder_id", "STRING", true, "Unique policyholder identifier."),
      f("birth_year", "INTEGER", false, "Year of birth, used for age banding."),
      f("region", "STRING", false, "Geographic region of the policyholder."),
      f("risk_segment", "STRING", false, "Underwriting risk segment — the loss-ratio cut."),
      f("tenure_years", "INTEGER", false, "Years the policyholder has been with the carrier."),
      f("acquisition_channel", "STRING", false, "Channel that acquired the policyholder."),
    ], "One row per policyholder with risk segment and tenure."),
    mart("dim_product", "Product", "TABLE", [
      f("product_id", "STRING", true, "Unique product identifier."),
      f("line", "STRING", false, "Line of business: auto, home, liability."),
      f("name", "STRING", false, "Product display name."),
      f("base_annual_premium", "NUMERIC", false, "Reference annual premium before rating factors."),
    ], "Reference of insurance products by line of business."),
    mart("dim_agent", "Agent", "TABLE", [
      f("agent_id", "STRING", true, "Unique agent identifier."),
      f("name", "STRING", false, "Agent or agency name."),
      f("channel", "STRING", false, "Distribution channel: direct, broker, agency."),
      f("region", "STRING", false, "Region the agent covers."),
      f("commission_rate", "FLOAT", false, "Standard commission rate for the agent."),
    ], "Reference of distribution agents and channels."),
    mart("fct_quotes", "Quotes", "VIEW", [
      f("quote_id", "STRING", true, "Unique quote identifier."),
      f("policyholder_id", "STRING", false, "Prospect the quote was issued to."),
      f("product_id", "STRING", false, "Product being quoted."),
      f("agent_id", "STRING", false, "Agent who issued the quote."),
      f("quoted_at", "TIMESTAMP", false, "When the quote was issued."),
      f("premium_quoted", "NUMERIC", false, "Annual premium offered."),
      f("is_bound", "BOOLEAN", false, "Whether the quote converted into a policy — the hit ratio."),
      f("bound_at", "DATE", false, "Date the quote was bound, if it was."),
      f("decline_reason", "STRING", false, "Why the prospect walked away, when known."),
    ], "One row per quote. Quote-to-bind (hit) ratio by product and channel."),
    mart("fct_policies", "Policies", "VIEW", [
      f("policy_id", "STRING", true, "Unique policy identifier."),
      f("quote_id", "STRING", false, "Quote the policy originated from."),
      f("policyholder_id", "STRING", false, "Insured policyholder."),
      f("product_id", "STRING", false, "Product underwritten."),
      f("effective_date", "DATE", false, "Coverage start date."),
      f("expiry_date", "DATE", false, "Coverage end date — the renewal moment."),
      f("annual_premium", "NUMERIC", false, "Written annual premium on the policy."),
      f("status", "STRING", false, "Current policy status (in-force, lapsed, cancelled)."),
      f("is_renewal", "BOOLEAN", false, "Whether this term is a renewal of a prior policy."),
    ], "One row per policy term. Renewals and in-force book."),
    mart("fct_premiums_monthly", "Premiums (monthly)", "VIEW", [
      f("premium_id", "STRING", true, "Unique identifier for the monthly premium record."),
      f("policy_id", "STRING", false, "Policy the record belongs to."),
      f("period_month", "DATE", false, "Calendar month of the record."),
      f("written_premium", "NUMERIC", false, "Premium written in the month."),
      f("earned_premium", "NUMERIC", false, "Premium earned in the month — the loss-ratio denominator."),
      f("commission_expense", "NUMERIC", false, "Commission paid out for the month."),
      f("admin_expense", "NUMERIC", false, "Allocated administrative expense — combined-ratio input."),
    ], "One row per policy × month. Earned premium and expense load."),
    mart("fct_claims", "Claims", "VIEW", [
      f("claim_id", "STRING", true, "Unique claim identifier."),
      f("policy_id", "STRING", false, "Policy the claim is filed against."),
      f("reported_at", "DATE", false, "Date the claim was reported."),
      f("loss_date", "DATE", false, "Date the insured loss occurred."),
      f("closed_at", "DATE", false, "Date the claim was closed."),
      f("status", "STRING", false, "Current claim status."),
      f("cause", "STRING", false, "Cause of loss (collision, fire, water, theft)."),
      f("reserve_amount", "NUMERIC", false, "Outstanding reserve held for the claim."),
      f("cycle_days", "INTEGER", false, "Report-to-close duration — claims cycle time."),
    ], "One row per claim. Cycle time, reserves and cause of loss."),
    mart("fct_claim_payments", "Claim Payments", "VIEW", [
      f("payment_id", "STRING", true, "Unique payment identifier."),
      f("claim_id", "STRING", false, "Claim the payment settles."),
      f("paid_at", "DATE", false, "Date the payment was made."),
      f("payment_type", "STRING", false, "Indemnity to the insured or loss-adjustment expense."),
      f("amount", "NUMERIC", false, "Amount paid — incurred losses, the loss-ratio numerator."),
      f("leakage_amount", "NUMERIC", false, "Overpayment identified in claims audit — leakage."),
    ], "One row per claim payment. Incurred losses and leakage."),
  ],
  edges: [
    rel("e1", "fct_quotes", "dim_policyholder", "policyholder_id", "policyholder_id"),
    rel("e2", "fct_quotes", "dim_product", "product_id", "product_id"),
    rel("e3", "fct_quotes", "dim_agent", "agent_id", "agent_id"),
    rel("e4", "fct_policies", "fct_quotes", "quote_id", "quote_id"),
    rel("e5", "fct_policies", "dim_policyholder", "policyholder_id", "policyholder_id"),
    rel("e6", "fct_policies", "dim_product", "product_id", "product_id"),
    rel("e7", "fct_premiums_monthly", "fct_policies", "policy_id", "policy_id"),
    rel("e8", "fct_claims", "fct_policies", "policy_id", "policy_id"),
    rel("e9", "fct_claim_payments", "fct_claims", "claim_id", "claim_id"),
  ],
};

export const insurance: Template = {
  id: "insurance",
  nicheId: "insurance",
  category: "industry",
  name: "Insurance (P&C)",
  description: "Underwriting & claims: policyholders, products, agents, quotes (hit ratio), policies & renewals, earned premium, claims and payments.",
  graph,
};
