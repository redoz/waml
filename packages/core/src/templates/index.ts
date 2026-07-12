// Template library. Stage 1b ships exactly one template — Orders Domain — as the
// living demo of the uml-domain profile. Its id is immutable: `?template=<id>`
// deep links are the CTA target for the blog gallery, launch emails and posts.
export type { Template } from "./helpers";

import type { Template } from "./helpers";
import { ordersDomain } from "./orders-domain";

export const TEMPLATES: Template[] = [ordersDomain];

export const INDUSTRY_TEMPLATES: Template[] = TEMPLATES.filter(t => t.category === "industry");
export const DATASET_TEMPLATES: Template[] = TEMPLATES.filter(t => t.category === "dataset");
