// Template library. Stage 1b ships exactly one template — Orders Domain — as the
// living demo of the uml-domain profile, committed as an `.okf` bundle. Its id is
// immutable: `?template=<id>` deep links are the CTA target for the blog gallery,
// launch emails and posts.
export type { Template } from "./helpers";

import type { Template } from "./helpers";
import { ordersDomainBundle } from "./orders-domain.bundle";

export const ordersDomain: Template = {
  id: "uml_orders_domain",
  nicheId: null,
  category: "dataset",
  name: "Orders Domain (UML)",
  description:
    "DDD-flavored UML domain model: aggregate root, entities, value objects, an enum and a service interface.",
  bundle: ordersDomainBundle,
};

export const TEMPLATES: Template[] = [ordersDomain];

export const INDUSTRY_TEMPLATES: Template[] = TEMPLATES.filter((t) => t.category === "industry");
export const DATASET_TEMPLATES: Template[] = TEMPLATES.filter((t) => t.category === "dataset");
