import type { ModelGraph } from "@mc/okf";
import { f, mart, rel, type Template } from "./helpers";

// On-demand Delivery / Logistics — last-mile model. fct_orders is demand;
// fct_deliveries the operational execution (assign → pickup → dropoff) with
// batching and payout; fct_courier_shifts the supply side by zone & hour.
// Zone is the conformed geography every branch shares.
//
// Goal coverage (niche "delivery_logistics"):
//   on-time at peak         → fct_deliveries (is_on_time) × fct_courier_shifts.hour_of_day
//   cost per delivery       → fct_deliveries (courier_payout, is_batched, batch_size)
//   time-to-assign & E2E    → fct_deliveries (time_to_assign_secs, delivery_mins)
//   cancellations/failures  → fct_cancellations (stage, is_failed_delivery)
//   supply vs demand        → fct_courier_shifts × fct_orders by dim_zone and hour
const graph: ModelGraph = {
  diagrams: [],
  nodes: [
    mart("dim_customer", "Customer", "VIEW", [
      f("customer_id", "STRING", true, "Unique customer identifier."),
      f("signup_date", "DATE", false, "Date the customer registered."),
      f("acquisition_channel", "STRING", false, "Channel that brought the customer in."),
      f("home_zone_id", "STRING", false, "Zone where the customer usually orders."),
      f("lifetime_orders", "INTEGER", false, "Total orders placed to date."),
      f("is_subscriber", "BOOLEAN", false, "Whether the customer holds a delivery subscription."),
    ], "One row per ordering customer."),
    mart("dim_courier", "Courier", "VIEW", [
      f("courier_id", "STRING", true, "Unique courier identifier."),
      f("onboarded_at", "DATE", false, "Date the courier was onboarded."),
      f("vehicle_type", "STRING", false, "Vehicle the courier rides (bike, scooter, car)."),
      f("home_zone_id", "STRING", false, "Zone the courier usually works."),
      f("rating", "FLOAT", false, "Average customer rating of the courier."),
      f("is_active", "BOOLEAN", false, "Whether the courier is currently active on the platform."),
    ], "Supply side: one row per courier."),
    mart("dim_merchant", "Merchant", "VIEW", [
      f("merchant_id", "STRING", true, "Unique merchant identifier."),
      f("name", "STRING", false, "Merchant display name."),
      f("category", "STRING", false, "Merchant vertical (restaurant, grocery, pharmacy)."),
      f("zone_id", "STRING", false, "Zone the merchant is located in."),
      f("avg_prep_mins", "INTEGER", false, "Typical order preparation time — the pickup-wait driver."),
    ], "One row per merchant/pickup point."),
    mart("dim_zone", "Zone", "TABLE", [
      f("zone_id", "STRING", true, "Unique zone identifier."),
      f("city", "STRING", false, "City the zone belongs to."),
      f("zone_name", "STRING", false, "Human-readable zone name."),
      f("area_km2", "FLOAT", false, "Zone area in square kilometres — density denominator."),
    ], "Reference of delivery zones — the geography supply/demand balance is judged on."),
    mart("fct_orders", "Orders", "VIEW", [
      f("order_id", "STRING", true, "Unique order identifier."),
      f("customer_id", "STRING", false, "Customer who placed the order."),
      f("merchant_id", "STRING", false, "Merchant preparing the order."),
      f("ordered_at", "TIMESTAMP", false, "When the order was placed — demand by zone and hour."),
      f("promised_at", "TIMESTAMP", false, "Delivery time promised at checkout — the on-time yardstick."),
      f("basket_value", "NUMERIC", false, "Value of the goods in the order."),
      f("delivery_fee", "NUMERIC", false, "Delivery fee charged to the customer."),
      f("status", "STRING", false, "Current order status."),
    ], "One row per order — the demand stream."),
    mart("fct_deliveries", "Deliveries", "VIEW", [
      f("delivery_id", "STRING", true, "Unique delivery identifier."),
      f("order_id", "STRING", false, "Order being delivered."),
      f("courier_id", "STRING", false, "Courier who carried the delivery."),
      f("assigned_at", "TIMESTAMP", false, "When a courier was assigned."),
      f("picked_up_at", "TIMESTAMP", false, "When the courier collected the order."),
      f("delivered_at", "TIMESTAMP", false, "When the order reached the customer."),
      f("time_to_assign_secs", "INTEGER", false, "Order → courier-assignment latency."),
      f("delivery_mins", "INTEGER", false, "End-to-end delivery time in minutes."),
      f("distance_km", "FLOAT", false, "Distance ridden for the delivery."),
      f("is_on_time", "BOOLEAN", false, "Whether the order arrived by the promised time."),
      f("is_batched", "BOOLEAN", false, "Whether the delivery was stacked with others — the routing cost lever."),
      f("batch_size", "INTEGER", false, "Number of orders in the batch."),
      f("courier_payout", "NUMERIC", false, "What the courier was paid — the cost-per-delivery numerator."),
    ], "One row per delivery execution. On-time, batching and cost."),
    mart("fct_cancellations", "Cancellations", "VIEW", [
      f("cancellation_id", "STRING", true, "Unique cancellation identifier."),
      f("order_id", "STRING", false, "Order that was cancelled or failed."),
      f("cancelled_at", "TIMESTAMP", false, "When the cancellation happened."),
      f("stage", "STRING", false, "Order stage at cancellation (pre-assign, pre-pickup, in-transit)."),
      f("cancelled_by", "STRING", false, "Who cancelled: customer, merchant, courier or platform."),
      f("reason", "STRING", false, "Stated cancellation reason."),
      f("is_failed_delivery", "BOOLEAN", false, "True when the courier could not complete the drop-off."),
    ], "One row per cancelled or failed order — where and why fills break."),
    mart("fct_courier_shifts", "Courier Shifts", "VIEW", [
      f("shift_id", "STRING", true, "Unique identifier for the courier's shift-hour record."),
      f("courier_id", "STRING", false, "Courier on shift."),
      f("zone_id", "STRING", false, "Zone the courier worked."),
      f("shift_date", "DATE", false, "Calendar day of the shift."),
      f("hour_of_day", "INTEGER", false, "Hour bucket — supply by zone and hour."),
      f("online_mins", "INTEGER", false, "Minutes the courier was online in the hour."),
      f("deliveries_completed", "INTEGER", false, "Deliveries completed in the hour."),
      f("utilization_pct", "FLOAT", false, "Share of online time spent on active deliveries."),
      f("surge_multiplier", "FLOAT", false, "Payout multiplier in effect — the supply-shaping lever."),
    ], "One row per courier × zone × hour. The supply side of the balance."),
  ],
  edges: [
    rel("e1", "dim_customer", "dim_zone", "home_zone_id", "zone_id"),
    rel("e2", "dim_merchant", "dim_zone", "zone_id", "zone_id"),
    rel("e3", "fct_orders", "dim_customer", "customer_id", "customer_id"),
    rel("e4", "fct_orders", "dim_merchant", "merchant_id", "merchant_id"),
    rel("e5", "fct_deliveries", "fct_orders", "order_id", "order_id"),
    rel("e6", "fct_deliveries", "dim_courier", "courier_id", "courier_id"),
    rel("e7", "fct_cancellations", "fct_orders", "order_id", "order_id"),
    rel("e8", "fct_courier_shifts", "dim_courier", "courier_id", "courier_id"),
    rel("e9", "fct_courier_shifts", "dim_zone", "zone_id", "zone_id"),
  ],
};

export const delivery_logistics: Template = {
  id: "delivery_logistics",
  nicheId: "delivery_logistics",
  category: "industry",
  name: "Delivery / Logistics",
  description: "Last-mile operations: customers, couriers & shifts, merchants, zones, orders, delivery execution (on-time, batching) and cancellations.",
  graph,
};
