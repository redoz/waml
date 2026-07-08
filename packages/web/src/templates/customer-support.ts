import type { ModelGraph } from "@mc/okf";
import { f, mart, rel, type Template } from "./helpers";

// Customer Support / Service — service-desk model, the shape of a Zendesk
// rollup. fct_tickets is the case header (SLA, AHT, FCR); fct_ticket_events its
// movement stream; CSAT hangs off the ticket; shifts and backlog snapshots give
// the staffing-vs-volume balance.
//
// Goal coverage (niche "customer_support"):
//   SLA hit rate            → fct_tickets (is_sla_breached, first_response_at) × dim_channel SLAs
//   CSAT                    → fct_csat_responses (score, is_negative) × dim_agent
//   AHT                     → fct_tickets.handle_time_mins by category and channel
//   FCR & reopens           → fct_tickets (is_fcr, reopen_count) + fct_ticket_events
//   staffing vs volume      → fct_agent_shifts × fct_backlog_snapshots by channel
const graph: ModelGraph = {
  storageId: null,
  nodes: [
    mart("dim_customer", "Customer", "VIEW", [
      f("customer_id", "STRING", true, "Unique customer identifier."),
      f("plan_tier", "STRING", false, "Support entitlement tier of the customer."),
      f("region", "STRING", false, "Customer's region and time zone bucket."),
      f("signup_date", "DATE", false, "When the customer joined."),
      f("lifetime_tickets", "INTEGER", false, "Tickets opened to date — heavy-user flag."),
    ], "One row per customer with entitlement tier."),
    mart("dim_agent", "Agent", "TABLE", [
      f("agent_id", "STRING", true, "Unique agent identifier."),
      f("name", "STRING", false, "Agent's name."),
      f("team", "STRING", false, "Team or queue the agent works."),
      f("hired_at", "DATE", false, "When the agent joined."),
      f("seniority", "STRING", false, "Agent seniority band (junior/senior/lead)."),
    ], "One row per support agent."),
    mart("dim_channel", "Channel", "TABLE", [
      f("channel_id", "STRING", true, "Unique channel identifier."),
      f("name", "STRING", false, "Contact channel: email, chat, phone, portal."),
      f("sla_first_response_mins", "INTEGER", false, "First-response SLA target for the channel."),
      f("sla_resolution_hours", "INTEGER", false, "Resolution SLA target for the channel."),
    ], "Reference of support channels with their SLA targets."),
    mart("fct_tickets", "Tickets", "VIEW", [
      f("ticket_id", "STRING", true, "Unique ticket identifier."),
      f("customer_id", "STRING", false, "Customer who opened the ticket."),
      f("agent_id", "STRING", false, "Agent who owns the ticket."),
      f("channel_id", "STRING", false, "Channel the ticket arrived on."),
      f("opened_at", "TIMESTAMP", false, "When the ticket was opened."),
      f("first_response_at", "TIMESTAMP", false, "First agent reply — the first-response SLA clock."),
      f("resolved_at", "TIMESTAMP", false, "When the ticket was resolved."),
      f("priority", "STRING", false, "Ticket priority level."),
      f("category", "STRING", false, "Topic classification of the ticket."),
      f("status", "STRING", false, "Current ticket status."),
      f("is_sla_breached", "BOOLEAN", false, "Whether any SLA target was missed."),
      f("is_fcr", "BOOLEAN", false, "Resolved on first contact — the FCR flag."),
      f("reopen_count", "INTEGER", false, "Times the ticket was reopened after resolution."),
      f("handle_time_mins", "INTEGER", false, "Total agent handling time — AHT."),
    ], "One row per ticket. SLA, AHT, FCR and reopens."),
    mart("fct_ticket_events", "Ticket Events", "CONNECTOR", [
      f("event_id", "STRING", true, "Unique event identifier."),
      f("ticket_id", "STRING", false, "Ticket the event belongs to."),
      f("event_ts", "TIMESTAMP", false, "When the event happened."),
      f("event_type", "STRING", false, "Kind of event: reply, escalation, transfer, reopen."),
      f("actor", "STRING", false, "Who acted: agent, customer or bot."),
    ], "One row per ticket event — the movement stream behind escalations and reopens."),
    mart("fct_csat_responses", "CSAT Responses", "VIEW", [
      f("response_id", "STRING", true, "Unique survey-response identifier."),
      f("ticket_id", "STRING", false, "Ticket the rating refers to."),
      f("submitted_at", "TIMESTAMP", false, "When the rating was submitted."),
      f("score", "INTEGER", false, "CSAT score given by the customer."),
      f("is_negative", "BOOLEAN", false, "Detractor response — the negative-response rate numerator."),
    ], "One row per CSAT survey response."),
    mart("fct_agent_shifts", "Agent Shifts", "VIEW", [
      f("shift_id", "STRING", true, "Unique identifier for the agent-day record."),
      f("agent_id", "STRING", false, "Agent on shift."),
      f("shift_date", "DATE", false, "Calendar day of the shift."),
      f("scheduled_mins", "INTEGER", false, "Minutes the agent was scheduled."),
      f("handled_tickets", "INTEGER", false, "Tickets handled during the shift."),
      f("occupancy_pct", "FLOAT", false, "Share of scheduled time spent on tickets — staffing pressure."),
    ], "One row per agent × day. The supply side of staffing balance."),
    mart("fct_backlog_snapshots", "Backlog Snapshots", "VIEW", [
      f("snapshot_id", "STRING", true, "Unique identifier for the backlog snapshot."),
      f("snapshot_date", "DATE", false, "Day the backlog was measured."),
      f("channel_id", "STRING", false, "Channel the backlog belongs to."),
      f("open_tickets", "INTEGER", false, "Tickets open at snapshot time — the demand side."),
      f("oldest_ticket_age_hours", "INTEGER", false, "Age of the oldest open ticket — queue health."),
    ], "One row per channel × day of open-ticket backlog."),
  ],
  edges: [
    rel("e1", "fct_tickets", "dim_customer", "customer_id", "customer_id"),
    rel("e2", "fct_tickets", "dim_agent", "agent_id", "agent_id"),
    rel("e3", "fct_tickets", "dim_channel", "channel_id", "channel_id"),
    rel("e4", "fct_ticket_events", "fct_tickets", "ticket_id", "ticket_id"),
    rel("e5", "fct_csat_responses", "fct_tickets", "ticket_id", "ticket_id"),
    rel("e6", "fct_agent_shifts", "dim_agent", "agent_id", "agent_id"),
    rel("e7", "fct_backlog_snapshots", "dim_channel", "channel_id", "channel_id"),
  ],
};

export const customer_support: Template = {
  id: "customer_support",
  nicheId: "customer_support",
  category: "industry",
  name: "Customer Support / Service",
  description: "Service desk: customers, agents & shifts, channels with SLA targets, tickets (AHT/FCR), ticket events, CSAT and backlog snapshots.",
  graph,
};
