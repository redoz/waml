// A template is a committed `.okf` bundle plus its gallery metadata. The WASM
// core derives its Model from the bundle. (The old `ModelGraph`-authoring helpers
// — cls/attr/edge/enumOf/f/node/rel — retired with the TS serialize path.)
export interface Template {
  id: string;                    // immutable — ?template=<id> deep links are public CTAs
  nicheId: string | null;
  category: "industry" | "dataset";
  name: string;
  description: string;
  /** The template as a committed `.okf` bundle (`[path, markdown][]`) — the WASM
   *  core derives its Model. */
  bundle: [string, string][];
}
