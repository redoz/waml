import type { ModelGraph } from "@mc/okf";
import { f, mart, rel, type Template } from "./helpers";

// Mobile / Gaming — free-to-play telemetry model. fct_sessions/fct_events are
// the high-volume engagement streams (retention, FTUE funnel); monetization
// splits into fct_iap_purchases (ARPPU, payer conversion) and fct_ad_impressions
// (ad ARPDAU). fct_ua_spend closes the loop on CPI and D7 ROAS by campaign.
//
// Goal coverage (niche "mobile_gaming"):
//   D1/D7/D30 retention     → fct_sessions.day_number cohorts by dim_campaign
//   ARPDAU                  → fct_iap_purchases + fct_ad_impressions ÷ daily actives
//   payer conversion/ARPPU  → fct_iap_purchases (is_first_purchase) × dim_product
//   CPI & D7 ROAS           → fct_ua_spend (spend, installs) × player LTV by campaign
//   FTUE funnel             → fct_events (event_name, level) step-through rates
const graph: ModelGraph = {
  diagrams: [],
  nodes: [
    mart("dim_player", "Player", "VIEW", [
      f("player_id", "STRING", true, "Unique player identifier."),
      f("install_ts", "TIMESTAMP", false, "When the player first installed the app."),
      f("platform", "STRING", false, "Device platform (iOS / Android)."),
      f("country", "STRING", false, "Player's country."),
      f("acquisition_source", "STRING", false, "Channel that brought the player in."),
      f("campaign_id", "STRING", false, "UA campaign that acquired the player — joins to spend via the campaign dim."),
      f("is_payer", "BOOLEAN", false, "Whether the player has ever paid."),
      f("ltv", "NUMERIC", false, "Lifetime value to date."),
      f("last_active_date", "DATE", false, "Most recent day the player was active."),
    ], "One row per player/install. Acquisition, device and LTV state."),
    mart("dim_campaign", "UA Campaign", "TABLE", [
      f("campaign_id", "STRING", true, "Unique identifier for the user-acquisition campaign."),
      f("network", "STRING", false, "Ad network the campaign runs on."),
      f("campaign_name", "STRING", false, "Human-readable campaign name."),
      f("country", "STRING", false, "Country the campaign targets."),
      f("platform", "STRING", false, "Store platform targeted (iOS / Android)."),
    ], "Reference of UA campaigns — the join between spend and acquired players."),
    mart("dim_product", "IAP Catalog", "TABLE", [
      f("product_sku", "STRING", true, "Store SKU identifying the purchasable item."),
      f("name", "STRING", false, "Display name of the item."),
      f("product_type", "STRING", false, "Kind of item: currency pack, battle pass, cosmetic, bundle."),
      f("price_tier", "NUMERIC", false, "Standard store price point in USD."),
    ], "Reference of in-app purchase SKUs — ARPPU mix by product type."),
    mart("fct_sessions", "Sessions", "CONNECTOR", [
      f("session_id", "STRING", true, "Unique session identifier."),
      f("player_id", "STRING", false, "Player who played the session."),
      f("started_at", "TIMESTAMP", false, "Session start time."),
      f("ended_at", "TIMESTAMP", false, "Session end time."),
      f("session_length_secs", "INTEGER", false, "Session duration in seconds."),
      f("level_reached", "INTEGER", false, "Highest level reached in the session."),
      f("day_number", "INTEGER", false, "Days since install — powers D1/D7/D30 retention."),
    ], "One row per game session. Engagement, retention and session length."),
    mart("fct_events", "Events", "CONNECTOR", [
      f("event_id", "STRING", true, "Unique event identifier."),
      f("player_id", "STRING", false, "Player who triggered the event."),
      f("session_id", "STRING", false, "Session the event belongs to."),
      f("event_ts", "TIMESTAMP", false, "When the event occurred."),
      f("event_name", "STRING", false, "tutorial_step / level_complete / store_open …"),
      f("level", "INTEGER", false, "Game level at the time of the event."),
      f("value", "FLOAT", false, "Numeric value attached to the event."),
    ], "One row per gameplay/telemetry event. FTUE funnel and feature usage."),
    mart("fct_iap_purchases", "IAP Purchases", "VIEW", [
      f("purchase_id", "STRING", true, "Unique purchase identifier."),
      f("player_id", "STRING", false, "Player who made the purchase."),
      f("purchased_at", "TIMESTAMP", false, "When the purchase was made."),
      f("product_sku", "STRING", false, "Purchased product identifier."),
      f("price_usd", "NUMERIC", false, "Purchase price in USD."),
      f("currency", "STRING", false, "Currency the player paid in."),
      f("store", "STRING", false, "App store where the purchase was made."),
      f("is_first_purchase", "BOOLEAN", false, "Payer-conversion event."),
    ], "One row per in-app purchase. Monetization, ARPPU and payer conversion."),
    mart("fct_ad_impressions", "Ad Impressions", "CONNECTOR", [
      f("impression_id", "STRING", true, "Unique ad impression identifier."),
      f("player_id", "STRING", false, "Player who saw the ad."),
      f("shown_at", "TIMESTAMP", false, "When the ad was shown."),
      f("ad_format", "STRING", false, "rewarded / interstitial / banner."),
      f("placement", "STRING", false, "Where in the app the ad was placed."),
      f("revenue_usd", "NUMERIC", false, "Estimated ad revenue — ad ARPDAU."),
      f("network", "STRING", false, "Ad network serving the impression."),
    ], "One row per ad impression. Ad monetization and ARPDAU."),
    mart("fct_ua_spend", "UA Spend", "VIEW", [
      f("spend_id", "STRING", true, "Unique spend record identifier."),
      f("spend_date", "DATE", false, "Date the spend occurred."),
      f("network", "STRING", false, "Ad network the spend went to."),
      f("campaign_id", "STRING", false, "UA campaign the spend belongs to."),
      f("country", "STRING", false, "Country targeted by the spend."),
      f("installs", "INTEGER", false, "Installs attributed to the spend."),
      f("spend_usd", "NUMERIC", false, "Amount spent in USD."),
      f("impressions", "INTEGER", false, "Ad impressions bought."),
      f("clicks", "INTEGER", false, "Clicks generated."),
    ], "One row per campaign × day of user-acquisition spend. CPI and D7 ROAS."),
  ],
  edges: [
    rel("e1", "fct_sessions", "dim_player", "player_id", "player_id"),
    rel("e2", "fct_events", "dim_player", "player_id", "player_id"),
    rel("e3", "fct_events", "fct_sessions", "session_id", "session_id"),
    rel("e4", "fct_iap_purchases", "dim_player", "player_id", "player_id"),
    rel("e5", "fct_ad_impressions", "dim_player", "player_id", "player_id"),
    rel("e6", "fct_ua_spend", "dim_campaign", "campaign_id", "campaign_id"),
    rel("e7", "dim_player", "dim_campaign", "campaign_id", "campaign_id"),
    rel("e8", "fct_iap_purchases", "dim_product", "product_sku", "product_sku"),
  ],
};

export const mobile_gaming: Template = {
  id: "mobile_gaming",
  nicheId: "mobile_gaming",
  category: "industry",
  name: "Mobile / Gaming",
  description: "Free-to-play telemetry: players, UA campaigns & spend, sessions, events, IAP catalog & purchases, ad impressions.",
  graph,
};
