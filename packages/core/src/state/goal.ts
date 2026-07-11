export interface BusinessGoal {
  niche: string;
  goal: string;
}

export interface NichePreset {
  id: string;
  label: string;
  goals: string[];
}

// Twenty verticals with five sharp, metric-driven goals each — phrased as the
// levers a senior analyst would actually chase (named KPIs, cohorts and funnel
// stages), not generic aspirations. Used to seed the Business Goal picker.
export const NICHE_PRESETS: NichePreset[] = [
  {
    id: "ecommerce",
    label: "E-commerce / Retail",
    goals: [
      "Increase ROAS while holding CPC",
      "Lift contribution margin per order after shipping & returns",
      "Raise 90-day repeat-purchase rate and cohort LTV",
      "Cut return/refund rate on high-AOV categories",
      "Recover revenue lost to cart & checkout abandonment",
    ],
  },
  {
    id: "saas",
    label: "SaaS / Subscription",
    goals: [
      "Lift Net Revenue Retention (NRR) above 110%",
      "Cut logo & gross-revenue churn in the first 90 days",
      "Raise trial-to-paid conversion without discounting",
      "Shorten CAC payback below 12 months",
      "Grow expansion MRR from seat & usage upsell",
    ],
  },
  {
    id: "leadgen",
    label: "B2B Marketing / Lead-gen",
    goals: [
      "Improve MQL → SQL → Closed-Won conversion across the funnel",
      "Lower blended cost per SQL by channel",
      "Tie cross-channel touchpoints to closed-won revenue",
      "Shorten sales-cycle length for mid-market deals",
      "Raise pipeline velocity (deals × win-rate × ACV ÷ cycle)",
    ],
  },
  {
    id: "mobile_gaming",
    label: "Mobile App / Gaming",
    goals: [
      "Raise D1 / D7 / D30 retention curves",
      "Grow ARPDAU without hurting retention",
      "Lift payer conversion and ARPPU",
      "Lower CPI while holding D7 ROAS on install quality",
      "De-bottleneck the FTUE / onboarding funnel",
    ],
  },
  {
    id: "fintech_lending",
    label: "Fintech / Lending",
    goals: [
      "Raise approved → funded conversion (pull-through rate)",
      "Cut first-payment default & charge-off by origination cohort",
      "Improve fraud capture without raising false-positive declines",
      "Grow funded-account activation & deposit balances",
      "Lift portfolio yield / net interest margin per risk segment",
    ],
  },
  {
    id: "marketplace",
    label: "Marketplace / Platform",
    goals: [
      "Raise liquidity (search → transaction match rate)",
      "Improve supply utilization & seller activation",
      "Optimise take rate without suppressing GMV",
      "Cut fill-rate failures and time-to-match",
      "Grow repeat rate on both buyer & seller sides",
    ],
  },
  {
    id: "ott_media",
    label: "Subscription Media / OTT",
    goals: [
      "Cut subscriber churn with engagement-based retention",
      "Lift average watch time & content completion rate",
      "Improve content ROI (cost per retained viewer-hour)",
      "Raise free / ad-tier → paid conversion",
      "Reduce involuntary churn via failed-payment recovery",
    ],
  },
  {
    id: "delivery_logistics",
    label: "On-demand Delivery / Logistics",
    goals: [
      "Raise on-time delivery rate at peak load",
      "Cut cost per delivery through batching & routing",
      "Shorten time-to-assign and end-to-end delivery time",
      "Lower order cancellation & failed-delivery rate",
      "Balance courier supply vs demand by zone & hour",
    ],
  },
  {
    id: "healthcare",
    label: "Healthcare Provider",
    goals: [
      "Cut 30-day readmission rate",
      "Reduce appointment no-show & late-cancellation rate",
      "Shorten revenue-cycle AR days & lower denial rate",
      "Optimise length-of-stay and clinic/bed utilization",
      "Improve patient throughput (door-to-provider time)",
    ],
  },
  {
    id: "insurance",
    label: "Insurance (P&C)",
    goals: [
      "Lower loss ratio by underwriting segment",
      "Improve combined ratio (loss + expense)",
      "Raise policy renewal / retention rate",
      "Lift quote-to-bind (hit) ratio profitably",
      "Cut claims cycle time & claims leakage",
    ],
  },
  {
    id: "b2b_sales",
    label: "B2B Sales / RevOps",
    goals: [
      "Raise win rate on qualified pipeline without discount erosion",
      "Shorten sales-cycle length by de-bottlenecking pipeline stages",
      "Improve forecast accuracy & cut quarter-end slippage",
      "Lift rep quota attainment and shorten ramp time",
      "Grow average deal size (ACV) via multi-product deals",
    ],
  },
  {
    id: "customer_support",
    label: "Customer Support / Service",
    goals: [
      "Hit first-response & resolution SLA across channels",
      "Raise CSAT and cut negative-response rate",
      "Cut average handle time (AHT) without hurting quality",
      "Lift first-contact resolution & reduce reopens",
      "Balance agent staffing to ticket volume by hour & channel",
    ],
  },
  {
    id: "hr_people",
    label: "HR / People Analytics",
    goals: [
      "Cut time-to-fill and cost-per-hire for key roles",
      "Raise offer-acceptance rate by source & segment",
      "Lower first-year regrettable attrition",
      "Close pay-equity gaps across levels & functions",
      "Lift engagement (eNPS) and link it to attrition risk",
    ],
  },
  {
    id: "telecom",
    label: "Telecom / ISP",
    goals: [
      "Cut monthly churn in high-value subscriber segments",
      "Grow ARPU via plan upgrades & add-on attach",
      "Reduce churn driven by network incidents (QoS)",
      "Improve collections & cut involuntary disconnects",
      "Raise 5G/fiber migration rate profitably",
    ],
  },
  {
    id: "hospitality",
    label: "Hotels / Hospitality",
    goals: [
      "Lift RevPAR without sacrificing occupancy",
      "Shift OTA bookings to direct to cut commission cost",
      "Cut cancellation & no-show revenue loss",
      "Grow ancillary revenue per occupied room",
      "Optimise rate-plan & channel mix by season and segment",
    ],
  },
  {
    id: "restaurants",
    label: "Restaurants / QSR",
    goals: [
      "Grow same-store sales & average check",
      "Cut food-cost % via menu engineering",
      "Keep labor cost % on target against hourly sales",
      "Reduce aggregator commission drag with first-party orders",
      "Cut waste & shrink on high-cost ingredients",
    ],
  },
  {
    id: "edtech",
    label: "EdTech / E-learning",
    goals: [
      "Raise course completion & lesson-level progression",
      "Lift free-to-paid conversion on learner cohorts",
      "Cut subscriber churn in the first 30 days",
      "Improve assessment pass rate (learning outcomes)",
      "Grow engaged learning time per active learner",
    ],
  },
  {
    id: "travel_ota",
    label: "Travel / OTA",
    goals: [
      "Raise look-to-book conversion across devices",
      "Grow ancillary attach (bags, seats, insurance) per booking",
      "Cut cancellation & rebooking cost",
      "Improve take-rate & margin mix across suppliers",
      "Lift repeat-booking rate & app share of bookings",
    ],
  },
  {
    id: "retail_pos",
    label: "Retail Chain / POS & Inventory",
    goals: [
      "Grow same-store sales & basket size",
      "Cut stockouts on top-velocity SKUs",
      "Raise inventory turns & cut weeks-of-supply on slow movers",
      "Measure true promo uplift & kill margin-eroding promos",
      "Reduce shrinkage hot spots by store & category",
    ],
  },
  {
    id: "manufacturing",
    label: "Manufacturing / Supply Chain",
    goals: [
      "Raise OEE (availability × performance × quality) per line",
      "Cut unplanned downtime on bottleneck machines",
      "Lower defect & scrap rate per product and shift",
      "Improve supplier OTIF and inbound quality",
      "Hit production-plan attainment without overtime creep",
    ],
  },
];

const KEY = "mc.goal.v1";

export function loadGoal(): BusinessGoal | null {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw);
    if (parsed && typeof parsed.niche === "string" && typeof parsed.goal === "string") {
      return { niche: parsed.niche, goal: parsed.goal };
    }
    return null;
  } catch {
    return null;
  }
}

export function persistGoal(goal: BusinessGoal | null): void {
  try {
    if (goal === null) localStorage.removeItem(KEY);
    else localStorage.setItem(KEY, JSON.stringify(goal));
  } catch {
    // best-effort; ignore quota / private-mode failures
  }
}
