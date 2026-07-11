import type { ModelGraph } from "@uaml/okf";
import { f, mart, rel, type Template } from "./helpers";

// Subscription Media / OTT — streaming model. fct_plays is the high-volume
// engagement stream (watch time, completion); fct_subscription_events rebuilds
// the plan-movement waterfall; fct_payments carries the involuntary-churn loop;
// content costs against watch hours give content ROI.
//
// Goal coverage (niche "ott_media"):
//   churn vs engagement     → fct_engagement_daily × fct_subscription_events (cancel)
//   watch time & completion → fct_plays (watch_secs, completion_pct) × dim_content
//   content ROI             → dim_content (production/license cost) ÷ retained viewer-hours
//   free/ad → paid          → fct_subscription_events (plan_from → plan_to) × dim_plan.tier
//   involuntary churn       → fct_payments (is_failed, retry_count, recovered_at)
const graph: ModelGraph = {
  diagrams: [],
  nodes: [
    mart("dim_subscriber", "Subscriber", "VIEW", [
      f("subscriber_id", "STRING", true, "Unique subscriber identifier."),
      f("signup_date", "DATE", false, "Date the subscriber first registered."),
      f("acquisition_channel", "STRING", false, "Marketing channel that brought the subscriber in."),
      f("country", "STRING", false, "Subscriber's country."),
      f("current_plan_id", "STRING", false, "Plan the subscriber is on right now."),
      f("device_pref", "STRING", false, "Most-used device class (TV, mobile, web)."),
      f("is_active", "BOOLEAN", false, "Whether the subscription is currently active."),
      f("churned_at", "DATE", false, "Date the subscriber churned, if they did."),
    ], "One row per subscriber with plan, device and churn state."),
    mart("dim_plan", "Plan", "TABLE", [
      f("plan_id", "STRING", true, "Unique plan identifier."),
      f("name", "STRING", false, "Display name of the plan."),
      f("tier", "STRING", false, "free / ad-supported / premium — the conversion ladder."),
      f("monthly_price", "NUMERIC", false, "Monthly subscription price."),
      f("ads_included", "BOOLEAN", false, "Whether the plan shows ads."),
    ], "Reference of subscription plans and tiers."),
    mart("dim_content", "Content", "TABLE", [
      f("content_id", "STRING", true, "Unique content identifier."),
      f("title", "STRING", false, "Title of the show or movie."),
      f("content_type", "STRING", false, "Kind of content: series, movie, live event."),
      f("genre", "STRING", false, "Primary genre of the title."),
      f("release_date", "DATE", false, "When the title premiered on the platform."),
      f("runtime_mins", "INTEGER", false, "Runtime in minutes."),
      f("production_cost", "NUMERIC", false, "Production cost for originals — the content-ROI denominator."),
      f("license_cost", "NUMERIC", false, "Licensing cost for acquired titles."),
    ], "One row per title in the catalog with cost attributes."),
    mart("fct_subscription_events", "Subscription Events", "VIEW", [
      f("event_id", "STRING", true, "Unique subscription-event identifier."),
      f("subscriber_id", "STRING", false, "Subscriber the event belongs to."),
      f("event_ts", "TIMESTAMP", false, "When the plan change occurred."),
      f("event_type", "STRING", false, "new / upgrade / downgrade / cancel / reactivate."),
      f("plan_from", "STRING", false, "Plan before the change — free→paid conversion reads from here."),
      f("plan_to", "STRING", false, "Plan after the change."),
      f("mrr_delta", "NUMERIC", false, "Signed change in monthly recurring revenue."),
    ], "One row per plan movement. Conversion ladder and churn waterfall."),
    mart("fct_plays", "Plays", "CONNECTOR", [
      f("play_id", "STRING", true, "Unique identifier for the playback session."),
      f("subscriber_id", "STRING", false, "Subscriber who watched."),
      f("content_id", "STRING", false, "Title that was played."),
      f("started_at", "TIMESTAMP", false, "When playback began."),
      f("watch_secs", "INTEGER", false, "Seconds actually watched."),
      f("completion_pct", "FLOAT", false, "Share of the title watched — completion rate."),
      f("device", "STRING", false, "Device the playback ran on."),
    ], "One row per playback session — watch time and completion."),
    mart("fct_payments", "Payments", "VIEW", [
      f("payment_id", "STRING", true, "Unique payment identifier."),
      f("subscriber_id", "STRING", false, "Subscriber billed."),
      f("billed_at", "DATE", false, "Date the charge was attempted."),
      f("amount", "NUMERIC", false, "Amount charged."),
      f("status", "STRING", false, "Payment status (paid, failed, refunded)."),
      f("is_failed", "BOOLEAN", false, "Failed charge — the involuntary-churn trigger."),
      f("retry_count", "INTEGER", false, "Dunning retries attempted so far."),
      f("recovered_at", "DATE", false, "When a failed payment was recovered, if it was."),
    ], "One row per billing attempt. Failed-payment recovery and involuntary churn."),
    mart("fct_ad_impressions", "Ad Impressions", "CONNECTOR", [
      f("impression_id", "STRING", true, "Unique ad-impression identifier."),
      f("subscriber_id", "STRING", false, "Subscriber who saw the ad."),
      f("content_id", "STRING", false, "Title the ad ran against."),
      f("shown_at", "TIMESTAMP", false, "When the ad was shown."),
      f("ad_pod_position", "INTEGER", false, "Position of the ad within its pod."),
      f("cpm", "NUMERIC", false, "Effective CPM for the impression."),
      f("ad_revenue", "NUMERIC", false, "Revenue earned from the impression — ad-tier monetization."),
    ], "One row per ad impression on the ad-supported tier."),
    mart("fct_engagement_daily", "Engagement (daily)", "VIEW", [
      f("engagement_id", "STRING", true, "Unique identifier for the daily engagement record."),
      f("subscriber_id", "STRING", false, "Subscriber the record covers."),
      f("activity_date", "DATE", false, "Calendar day of activity."),
      f("sessions", "INTEGER", false, "Viewing sessions started that day."),
      f("watch_mins", "INTEGER", false, "Minutes watched that day — the retention-driving engagement signal."),
      f("titles_watched", "INTEGER", false, "Distinct titles watched that day."),
    ], "One row per subscriber × day. The engagement signal churn models feed on."),
  ],
  edges: [
    rel("e1", "dim_subscriber", "dim_plan", "current_plan_id", "plan_id"),
    rel("e2", "fct_subscription_events", "dim_subscriber", "subscriber_id", "subscriber_id"),
    rel("e3", "fct_plays", "dim_subscriber", "subscriber_id", "subscriber_id"),
    rel("e4", "fct_plays", "dim_content", "content_id", "content_id"),
    rel("e5", "fct_payments", "dim_subscriber", "subscriber_id", "subscriber_id"),
    rel("e6", "fct_ad_impressions", "dim_subscriber", "subscriber_id", "subscriber_id"),
    rel("e7", "fct_ad_impressions", "dim_content", "content_id", "content_id"),
    rel("e8", "fct_engagement_daily", "dim_subscriber", "subscriber_id", "subscriber_id"),
  ],
};

export const ott_media: Template = {
  id: "ott_media",
  nicheId: "ott_media",
  category: "industry",
  name: "Subscription Media / OTT",
  description: "Streaming analytics: subscribers & plans, content catalog with costs, plays, plan movements, payments (dunning) and ad-tier impressions.",
  graph,
};
