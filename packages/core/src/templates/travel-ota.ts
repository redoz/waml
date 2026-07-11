import type { ModelGraph } from "@uaml/okf";
import { f, mart, rel, type Template } from "./helpers";

// Travel / OTA — booking-funnel model. fct_searches is the shopping stream
// (look-to-book); fct_bookings the transaction with supplier take-rate;
// ancillaries attach to the booking; cancellations carry refund vs penalty;
// reviews close the loop on repeat behaviour.
//
// Goal coverage (niche "travel_ota"):
//   look-to-book            → fct_searches (clicked, converted) by device
//   ancillary attach        → fct_ancillaries per booking (type, amount, margin)
//   cancellation cost       → fct_cancellations (refund_amount, penalty_kept, is_rebooked)
//   take-rate & margin mix  → fct_bookings (take_rate_pct, commission_revenue) × dim_supplier
//   repeat & app share      → dim_traveler (bookings_count, is_app_user) + fct_bookings.booking_channel
const graph: ModelGraph = {
  diagrams: [],
  nodes: [
    mart("dim_traveler", "Traveler", "VIEW", [
      f("traveler_id", "STRING", true, "Unique traveler identifier."),
      f("signup_date", "DATE", false, "When the traveler created an account."),
      f("home_country", "STRING", false, "Traveler's home country."),
      f("acquisition_channel", "STRING", false, "Channel that acquired the traveler."),
      f("loyalty_tier", "STRING", false, "Loyalty-program tier."),
      f("bookings_count", "INTEGER", false, "Completed bookings to date — repeat-rate base."),
      f("is_app_user", "BOOLEAN", false, "Whether the traveler books through the app."),
    ], "One row per traveler with loyalty and app usage."),
    mart("dim_supplier", "Supplier", "TABLE", [
      f("supplier_id", "STRING", true, "Unique supplier identifier."),
      f("name", "STRING", false, "Supplier name."),
      f("supplier_type", "STRING", false, "Kind of supplier: airline, hotel, car, activity."),
      f("contract_type", "STRING", false, "merchant / agency — determines how margin is earned."),
      f("base_commission_pct", "FLOAT", false, "Contracted commission rate — the margin-mix input."),
    ], "Reference of travel suppliers and contract terms."),
    mart("dim_destination", "Destination", "TABLE", [
      f("destination_id", "STRING", true, "Unique destination identifier."),
      f("city", "STRING", false, "Destination city."),
      f("country", "STRING", false, "Destination country."),
      f("region", "STRING", false, "World region for rollups."),
      f("peak_month", "INTEGER", false, "Month demand peaks — seasonality marker."),
    ], "Reference of destinations with seasonality."),
    mart("fct_searches", "Searches", "CONNECTOR", [
      f("search_id", "STRING", true, "Unique search identifier."),
      f("traveler_id", "STRING", false, "Traveler who searched; null for anonymous shoppers."),
      f("destination_id", "STRING", false, "Destination searched for."),
      f("searched_at", "TIMESTAMP", false, "When the search ran."),
      f("product_type", "STRING", false, "Product shopped: flight, hotel, package, car."),
      f("device", "STRING", false, "Device used — look-to-book differs sharply by device."),
      f("results_count", "INTEGER", false, "Results returned."),
      f("clicked", "BOOLEAN", false, "Whether a result was clicked."),
      f("converted", "BOOLEAN", false, "Whether the search ended in a booking — look-to-book."),
    ], "One row per search — the shopping stream."),
    mart("fct_bookings", "Bookings", "VIEW", [
      f("booking_id", "STRING", true, "Unique booking identifier."),
      f("traveler_id", "STRING", false, "Traveler who booked."),
      f("supplier_id", "STRING", false, "Supplier fulfilled by."),
      f("destination_id", "STRING", false, "Destination of the trip."),
      f("booked_at", "TIMESTAMP", false, "When the booking was made."),
      f("travel_date", "DATE", false, "Trip start date."),
      f("product_type", "STRING", false, "Product booked: flight, hotel, package, car."),
      f("gross_value", "NUMERIC", false, "Gross booking value."),
      f("take_rate_pct", "FLOAT", false, "Share of gross value kept as revenue."),
      f("commission_revenue", "NUMERIC", false, "Revenue earned on the booking."),
      f("booking_channel", "STRING", false, "web / app — the app-share axis."),
      f("status", "STRING", false, "Current booking status."),
    ], "One row per booking. GBV, take rate and channel."),
    mart("fct_ancillaries", "Ancillaries", "VIEW", [
      f("line_id", "STRING", true, "Unique identifier for the ancillary line."),
      f("booking_id", "STRING", false, "Booking the ancillary attaches to."),
      f("ancillary_type", "STRING", false, "What was attached: bags, seat, insurance, transfer."),
      f("amount", "NUMERIC", false, "Price of the ancillary."),
      f("margin", "NUMERIC", false, "Margin earned on the ancillary — often the profit pool."),
    ], "One row per ancillary sold with a booking — attach rate and margin."),
    mart("fct_cancellations", "Cancellations", "VIEW", [
      f("cancellation_id", "STRING", true, "Unique cancellation identifier."),
      f("booking_id", "STRING", false, "Booking cancelled."),
      f("cancelled_at", "TIMESTAMP", false, "When the cancellation happened."),
      f("days_before_travel", "INTEGER", false, "Lead time between cancellation and travel date."),
      f("refund_amount", "NUMERIC", false, "Amount refunded to the traveler."),
      f("penalty_kept", "NUMERIC", false, "Penalty retained — offsets cancellation cost."),
      f("is_rebooked", "BOOLEAN", false, "Whether the traveler rebooked — saved revenue."),
    ], "One row per cancellation. Refund cost, penalties and rebooking."),
    mart("fct_reviews", "Reviews", "VIEW", [
      f("review_id", "STRING", true, "Unique review identifier."),
      f("booking_id", "STRING", false, "Booking the review refers to."),
      f("submitted_at", "DATE", false, "When the review was submitted."),
      f("rating", "INTEGER", false, "Star rating given."),
      f("nps_bucket", "STRING", false, "promoter / passive / detractor — repeat-booking predictor."),
    ], "One row per post-trip review."),
  ],
  edges: [
    rel("e1", "fct_searches", "dim_traveler", "traveler_id", "traveler_id"),
    rel("e2", "fct_searches", "dim_destination", "destination_id", "destination_id"),
    rel("e3", "fct_bookings", "dim_traveler", "traveler_id", "traveler_id"),
    rel("e4", "fct_bookings", "dim_supplier", "supplier_id", "supplier_id"),
    rel("e5", "fct_bookings", "dim_destination", "destination_id", "destination_id"),
    rel("e6", "fct_ancillaries", "fct_bookings", "booking_id", "booking_id"),
    rel("e7", "fct_cancellations", "fct_bookings", "booking_id", "booking_id"),
    rel("e8", "fct_reviews", "fct_bookings", "booking_id", "booking_id"),
  ],
};

export const travel_ota: Template = {
  id: "travel_ota",
  nicheId: "travel_ota",
  category: "industry",
  name: "Travel / OTA",
  description: "Booking funnel: travelers, suppliers with contract terms, destinations, searches (look-to-book), bookings, ancillaries, cancellations and reviews.",
  graph,
};
