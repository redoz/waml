import type { ModelGraph } from "@uaml/okf";
import { f, mart, rel, type Template } from "./helpers";

// Finance / Fintech — neobank + lending model. Two fact streams sit side by
// side: fct_transactions (card/money movement → engagement, interchange, fraud)
// and the lending funnel fct_loans → fct_repayments (origination, pull-through,
// DPD and charge-off). KYC/risk attributes live on the customer dimension.
//
// Goal coverage (niche "fintech_lending"):
//   pull-through rate       → fct_loans (approved_amount → funded_amount)
//   FPD & charge-off        → fct_repayments (days_past_due, is_charged_off) by funded_at cohort
//   fraud capture vs FP     → fct_transactions (fraud_score, is_declined, is_confirmed_fraud)
//   activation & deposits   → fct_accounts (activated_at) + fct_balances_monthly.avg_balance
//   yield / NIM per segment → fct_balances_monthly (interest_earned − interest_paid) × risk_band
const graph: ModelGraph = {
  diagrams: [],
  nodes: [
    mart("dim_customer", "Customer", "VIEW", [
      f("customer_id", "STRING", true, "Unique customer identifier."),
      f("signup_date", "DATE", false, "Date the customer signed up."),
      f("kyc_status", "STRING", false, "passed / pending / rejected."),
      f("risk_band", "STRING", false, "Internal risk tier assigned to the customer."),
      f("credit_score", "INTEGER", false, "Credit score at onboarding."),
      f("acquisition_channel", "STRING", false, "Channel that brought the customer in."),
      f("region", "STRING", false, "Customer's geographic region."),
      f("is_funded", "BOOLEAN", false, "Has at least one funded account — activation flag."),
    ], "One row per customer with KYC, risk band and acquisition."),
    mart("dim_product", "Product", "TABLE", [
      f("product_id", "STRING", true, "Unique product identifier."),
      f("name", "STRING", false, "Product display name."),
      f("product_type", "STRING", false, "deposit / card / loan / BNPL."),
      f("apr", "FLOAT", false, "Annual percentage rate for the product."),
      f("term_months", "INTEGER", false, "Product term length in months."),
    ], "Reference of financial products."),
    mart("fct_accounts", "Accounts", "VIEW", [
      f("account_id", "STRING", true, "Unique account identifier."),
      f("customer_id", "STRING", false, "Owning customer."),
      f("product_id", "STRING", false, "Product held in this account."),
      f("opened_at", "DATE", false, "Date the account was opened."),
      f("status", "STRING", false, "Current account status."),
      f("current_balance", "NUMERIC", false, "Current account balance."),
      f("activated_at", "DATE", false, "First funding / first card use."),
      f("is_active", "BOOLEAN", false, "Whether the account is currently active."),
    ], "One row per opened product holding. Balances and activation state."),
    mart("fct_transactions", "Transactions", "CONNECTOR", [
      f("txn_id", "STRING", true, "Unique transaction identifier."),
      f("account_id", "STRING", false, "Account the transaction belongs to."),
      f("txn_ts", "TIMESTAMP", false, "When the transaction occurred."),
      f("txn_type", "STRING", false, "Type of transaction."),
      f("mcc", "STRING", false, "Merchant category code."),
      f("amount", "NUMERIC", false, "Transaction amount."),
      f("currency", "STRING", false, "Currency of the transaction."),
      f("is_declined", "BOOLEAN", false, "Whether the transaction was declined."),
      f("fraud_score", "FLOAT", false, "Model score at authorization time."),
      f("is_confirmed_fraud", "BOOLEAN", false, "Post-investigation label — with fraud_score gives capture rate vs false-positive declines."),
      f("channel", "STRING", false, "Channel used for the transaction."),
    ], "One row per money movement / card authorization. Engagement, interchange and fraud."),
    mart("fct_loans", "Loans", "VIEW", [
      f("loan_id", "STRING", true, "Unique loan identifier."),
      f("customer_id", "STRING", false, "Borrowing customer."),
      f("product_id", "STRING", false, "Loan product applied for."),
      f("applied_at", "DATE", false, "Date the loan was applied for."),
      f("decision", "STRING", false, "approved / declined / withdrawn."),
      f("approved_amount", "NUMERIC", false, "Amount approved at underwriting."),
      f("funded_amount", "NUMERIC", false, "Approved → funded is the pull-through rate."),
      f("apr", "FLOAT", false, "Annual percentage rate on the loan."),
      f("term_months", "INTEGER", false, "Loan term length in months."),
      f("funded_at", "DATE", false, "Date the loan was funded."),
      f("status", "STRING", false, "Current loan status."),
    ], "One row per loan application → origination. Underwriting funnel and pull-through."),
    mart("fct_repayments", "Repayments", "VIEW", [
      f("repayment_id", "STRING", true, "Unique repayment identifier."),
      f("loan_id", "STRING", false, "Loan this repayment belongs to."),
      f("due_date", "DATE", false, "Date the payment is due."),
      f("paid_date", "DATE", false, "Date the payment was made."),
      f("due_amount", "NUMERIC", false, "Amount scheduled to be paid."),
      f("paid_amount", "NUMERIC", false, "Amount actually paid."),
      f("days_past_due", "INTEGER", false, "DPD bucket driver for delinquency."),
      f("is_charged_off", "BOOLEAN", false, "Whether the loan was charged off."),
    ], "One row per scheduled repayment. Delinquency (DPD) and charge-off."),
    mart("fct_balances_monthly", "Balances (monthly)", "VIEW", [
      f("snapshot_id", "STRING", true, "Unique identifier for the monthly balance snapshot."),
      f("account_id", "STRING", false, "Account the snapshot belongs to."),
      f("month", "DATE", false, "Calendar month of the snapshot."),
      f("avg_balance", "NUMERIC", false, "Average balance held over the month — deposit growth."),
      f("interest_earned", "NUMERIC", false, "Interest income earned on the account for the month."),
      f("interest_paid", "NUMERIC", false, "Interest paid out to the customer for the month."),
      f("fees", "NUMERIC", false, "Fee income collected on the account for the month."),
    ], "One row per account × month. Net interest margin and deposit balances."),
    mart("fct_collections", "Collections", "VIEW", [
      f("action_id", "STRING", true, "Unique identifier for the collections action."),
      f("loan_id", "STRING", false, "Delinquent loan being worked."),
      f("action_ts", "TIMESTAMP", false, "When the action was taken."),
      f("action_type", "STRING", false, "Kind of action: reminder, call, restructure, agency handoff."),
      f("outcome", "STRING", false, "Result of the action (promise-to-pay, paid, no contact)."),
      f("amount_recovered", "NUMERIC", false, "Money recovered by this action."),
    ], "One row per collections action on delinquent loans — recovery effectiveness."),
  ],
  edges: [
    rel("e1", "fct_accounts", "dim_customer", "customer_id", "customer_id"),
    rel("e2", "fct_accounts", "dim_product", "product_id", "product_id"),
    rel("e3", "fct_transactions", "fct_accounts", "account_id", "account_id"),
    rel("e4", "fct_loans", "dim_customer", "customer_id", "customer_id"),
    rel("e5", "fct_loans", "dim_product", "product_id", "product_id"),
    rel("e6", "fct_repayments", "fct_loans", "loan_id", "loan_id"),
    rel("e7", "fct_balances_monthly", "fct_accounts", "account_id", "account_id"),
    rel("e8", "fct_collections", "fct_loans", "loan_id", "loan_id"),
  ],
};

export const finance: Template = {
  id: "finance",
  nicheId: "fintech_lending",
  category: "industry",
  name: "Finance / Fintech",
  description: "Neobank & lending: customers (KYC/risk), accounts & balances (NIM), transactions with fraud labels, loans, repayments and collections.",
  graph,
};
