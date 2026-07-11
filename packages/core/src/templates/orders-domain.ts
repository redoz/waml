import type { ModelGraph } from "@uaml/okf";
import { attr, cls, enumOf, edge, type Template } from "./helpers";

// The spec's Order domain worked example — doubles as the living demo of the
// uml-domain profile (stereotype styles, composition, enum, value objects).
const graph: ModelGraph = {
  nodes: [
    cls("order", "Order", {
      stereotypes: ["aggregateRoot", "entity"],
      description: "A customer's placed order.",
      attributes: [
        attr("id", "OrderId"),
        attr("placedAt", "Timestamp"),
        attr("status", { name: "OrderStatus", ref: "order-status" }),
        attr("shippingAddress", { name: "Address", ref: "address" }, { mult: "0..1" }),
        attr("total", { name: "Money", ref: "money" }),
      ],
    }),
    cls("order-line", "OrderLine", {
      stereotypes: ["entity"],
      attributes: [
        attr("quantity", "Int"),
        attr("unitPrice", { name: "Money", ref: "money" }),
      ],
    }),
    cls("customer", "Customer", {
      stereotypes: ["aggregateRoot", "entity"],
      attributes: [attr("id", "CustomerId"), attr("name", "String"), attr("email", "Email")],
    }),
    enumOf("order-status", "OrderStatus", ["DRAFT", "PLACED", "SHIPPED", "CANCELLED"]),
    cls("money", "Money", {
      type: "uml.DataType", stereotypes: ["valueObject"],
      attributes: [attr("amount", "Decimal"), attr("currency", "CurrencyCode")],
    }),
    cls("address", "Address", {
      stereotypes: ["valueObject"],
      attributes: [attr("street", "String"), attr("city", "String"), attr("country", "CountryCode")],
    }),
    cls("pricing-service", "PricingService", { type: "uml.Interface", stereotypes: ["service"] }),
  ],
  edges: [
    edge("e1", "associates", "order", "customer", { multiplicity: "1", role: "order" }, { multiplicity: "1", role: "customer" }),
    edge("e2", "composes", "order", "order-line", { multiplicity: "1" }, { multiplicity: "1..*", role: "lines" }),
    edge("e3", "depends", "order", "pricing-service"),
  ],
  diagrams: [{
    key: "orders-domain",
    title: "Orders Domain Model",
    profile: "uml-domain",
    members: ["order", "order-line", "customer", "order-status", "money", "address", "pricing-service"],
  }],
};

export const ordersDomain: Template = {
  id: "uml_orders_domain",
  nicheId: null,
  category: "dataset",
  name: "Orders Domain (UML)",
  description: "DDD-flavored UML domain model: aggregate root, entities, value objects, an enum and a service interface.",
  graph,
};
