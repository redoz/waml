import type { ModelGraph } from "@uaml/okf";
import { f, mart, rel, type Template } from "./helpers";

// Hotels / Hospitality — revenue-management model. fct_reservations is the
// booking header (channel, commission, cancellation); fct_occupancy_daily the
// property rollup revenue managers read (RevPAR/ADR/occupancy); rate plans by
// day expose the pricing mix; ancillaries hang off the reservation.
//
// Goal coverage (niche "hospitality"):
//   RevPAR vs occupancy     → fct_occupancy_daily (revpar, adr, occupancy_pct)
//   OTA → direct shift      → fct_reservations.channel_id × dim_channel.commission_pct
//   cancellation & no-show  → fct_reservations (is_cancelled, is_no_show, lead_time_days)
//   ancillary per room      → fct_ancillary_revenue ÷ occupied rooms
//   rate-plan/channel mix   → fct_rate_plans_daily × fct_reservations by season/segment
const graph: ModelGraph = {
  diagrams: [],
  nodes: [
    mart("dim_property", "Property", "TABLE", [
      f("property_id", "STRING", true, "Unique property identifier."),
      f("name", "STRING", false, "Hotel name."),
      f("city", "STRING", false, "City the property is in."),
      f("star_rating", "INTEGER", false, "Star classification of the property."),
      f("rooms_total", "INTEGER", false, "Total sellable rooms."),
      f("segment", "STRING", false, "Property positioning: resort, business, boutique."),
    ], "One row per hotel property."),
    mart("dim_room_type", "Room Type", "TABLE", [
      f("room_type_id", "STRING", true, "Unique room-type identifier."),
      f("property_id", "STRING", false, "Property the room type belongs to."),
      f("name", "STRING", false, "Room-type name (standard, deluxe, suite)."),
      f("capacity", "INTEGER", false, "Maximum guests the room sleeps."),
      f("rooms_count", "INTEGER", false, "Number of rooms of this type — inventory."),
      f("base_rate", "NUMERIC", false, "Rack rate before dynamic pricing."),
    ], "One row per room type within a property."),
    mart("dim_guest", "Guest", "VIEW", [
      f("guest_id", "STRING", true, "Unique guest identifier."),
      f("first_stay_date", "DATE", false, "Date of the guest's first stay."),
      f("home_country", "STRING", false, "Guest's country of residence."),
      f("loyalty_tier", "STRING", false, "Loyalty-program tier."),
      f("stays_count", "INTEGER", false, "Completed stays to date — repeat-guest signal."),
      f("is_business", "BOOLEAN", false, "Whether the guest typically travels on business."),
    ], "One row per guest with loyalty and repeat behaviour."),
    mart("dim_channel", "Channel", "TABLE", [
      f("channel_id", "STRING", true, "Unique channel identifier."),
      f("name", "STRING", false, "Booking channel name (brand.com, Booking, Expedia, GDS)."),
      f("channel_type", "STRING", false, "direct / OTA / GDS / corporate."),
      f("commission_pct", "FLOAT", false, "Commission the channel takes — the direct-shift lever."),
    ], "Reference of booking channels with commission rates."),
    mart("fct_reservations", "Reservations", "VIEW", [
      f("reservation_id", "STRING", true, "Unique reservation identifier."),
      f("property_id", "STRING", false, "Property booked."),
      f("room_type_id", "STRING", false, "Room type booked."),
      f("guest_id", "STRING", false, "Guest who booked."),
      f("channel_id", "STRING", false, "Channel the booking came through."),
      f("booked_at", "TIMESTAMP", false, "When the reservation was made."),
      f("checkin_date", "DATE", false, "Scheduled arrival date."),
      f("nights", "INTEGER", false, "Number of nights booked."),
      f("adr", "NUMERIC", false, "Average daily rate on the reservation."),
      f("total_amount", "NUMERIC", false, "Total reservation value including taxes."),
      f("ota_commission", "NUMERIC", false, "Commission owed to the channel — the OTA cost."),
      f("lead_time_days", "INTEGER", false, "Booking-to-arrival lead time — cancellation driver."),
      f("status", "STRING", false, "Reservation status."),
      f("is_cancelled", "BOOLEAN", false, "Whether the reservation was cancelled."),
      f("is_no_show", "BOOLEAN", false, "Whether the guest failed to arrive."),
    ], "One row per reservation. Channel mix, commission, cancellations and no-shows."),
    mart("fct_occupancy_daily", "Occupancy (daily)", "VIEW", [
      f("occupancy_id", "STRING", true, "Unique identifier for the property-day record."),
      f("property_id", "STRING", false, "Property the record covers."),
      f("room_type_id", "STRING", false, "Room type the record covers."),
      f("stay_date", "DATE", false, "Calendar night."),
      f("rooms_occupied", "INTEGER", false, "Rooms sold that night."),
      f("rooms_available", "INTEGER", false, "Rooms available to sell."),
      f("occupancy_pct", "FLOAT", false, "rooms_occupied ÷ rooms_available."),
      f("adr", "NUMERIC", false, "Average daily rate achieved that night."),
      f("revpar", "NUMERIC", false, "Revenue per available room — adr × occupancy."),
    ], "One row per property × room type × night. RevPAR, ADR and occupancy."),
    mart("fct_ancillary_revenue", "Ancillary Revenue", "VIEW", [
      f("line_id", "STRING", true, "Unique identifier for the ancillary charge."),
      f("reservation_id", "STRING", false, "Reservation the charge was posted to."),
      f("posted_at", "DATE", false, "When the charge was posted."),
      f("category", "STRING", false, "Ancillary category: F&B, spa, parking, minibar."),
      f("amount", "NUMERIC", false, "Charge amount — ancillary revenue per stay."),
    ], "One row per ancillary charge posted to a reservation."),
    mart("fct_rate_plans_daily", "Rates (daily)", "VIEW", [
      f("rate_id", "STRING", true, "Unique identifier for the room-type-day rate record."),
      f("room_type_id", "STRING", false, "Room type the rate applies to."),
      f("rate_date", "DATE", false, "Stay date the rate is published for."),
      f("rate_plan", "STRING", false, "Rate plan name (BAR, non-refundable, corporate, package)."),
      f("published_rate", "NUMERIC", false, "Published nightly rate for the plan."),
      f("is_refundable", "BOOLEAN", false, "Whether the plan is refundable — the mix lever vs cancellations."),
    ], "One row per room type × date × rate plan. The pricing mix over the calendar."),
  ],
  edges: [
    rel("e1", "dim_room_type", "dim_property", "property_id", "property_id"),
    rel("e2", "fct_reservations", "dim_property", "property_id", "property_id"),
    rel("e3", "fct_reservations", "dim_room_type", "room_type_id", "room_type_id"),
    rel("e4", "fct_reservations", "dim_guest", "guest_id", "guest_id"),
    rel("e5", "fct_reservations", "dim_channel", "channel_id", "channel_id"),
    rel("e6", "fct_occupancy_daily", "dim_property", "property_id", "property_id"),
    rel("e7", "fct_occupancy_daily", "dim_room_type", "room_type_id", "room_type_id"),
    rel("e8", "fct_ancillary_revenue", "fct_reservations", "reservation_id", "reservation_id"),
    rel("e9", "fct_rate_plans_daily", "dim_room_type", "room_type_id", "room_type_id"),
  ],
};

export const hospitality: Template = {
  id: "hospitality",
  nicheId: "hospitality",
  category: "industry",
  name: "Hotels / Hospitality",
  description: "Revenue management: properties, room types, guests, channels with commissions, reservations, nightly occupancy (RevPAR/ADR), ancillaries and rates.",
  graph,
};
