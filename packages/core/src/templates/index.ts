// Template library. Ships four templates — one per UML diagram kind in the same
// Orders domain — each committed as an `.okf` bundle. `uml_orders_domain`'s id is
// immutable: `?template=<id>` deep links are the CTA target for the blog gallery,
// launch emails and posts. The three newer ids are free.
export type { Template } from "./helpers";

import type { Template } from "./helpers";
import { ordersDomainBundle } from "./orders-domain.bundle";
import { ordersCheckoutActivityBundle } from "./orders-checkout-activity.bundle";
import { ordersCheckoutSequenceBundle } from "./orders-checkout-sequence.bundle";
import { ordersUseCasesBundle } from "./orders-use-cases.bundle";

export const ordersDomain: Template = {
  id: "uml_orders_domain",
  nicheId: null,
  category: "dataset",
  name: "Orders Domain (UML)",
  description:
    "DDD-flavored UML domain model: aggregate root, entities, value objects, an enum and a service interface.",
  bundle: ordersDomainBundle,
};

export const ordersCheckoutActivity: Template = {
  id: "uml_orders_checkout_activity",
  nicheId: null,
  category: "dataset",
  name: "Orders Checkout (Activity)",
  description:
    "UML activity diagram of the checkout flow: actions, decisions, partitions and an Order object node.",
  bundle: ordersCheckoutActivityBundle,
};

export const ordersCheckoutSequence: Template = {
  id: "uml_orders_checkout_sequence",
  nicheId: null,
  category: "dataset",
  name: "Orders Checkout (Sequence)",
  description:
    "UML sequence diagram of placing an order: a Customer actor with Order and PricingService lifelines and a payment alt.",
  bundle: ordersCheckoutSequenceBundle,
};

export const ordersUseCases: Template = {
  id: "uml_orders_use_cases",
  nicheId: null,
  category: "dataset",
  name: "Orders Use Cases",
  description:
    "UML use-case diagram: a Customer actor with Place Order, Authenticate, Track Order and Cancel Order use cases (include / extend).",
  bundle: ordersUseCasesBundle,
};

export const TEMPLATES: Template[] = [
  ordersDomain,
  ordersCheckoutActivity,
  ordersCheckoutSequence,
  ordersUseCases,
];
