import { UML_DOMAIN } from "./umlDomain";

// A profile is pure data: render lens (emphasize/hide), stereotype → style map,
// and the palette the "add node" UI offers. Adding «saga» tomorrow = one line in
// a profile module, no renderer change.
export interface StereotypeStyle { header?: string; border?: "thick"; shape?: "hexagon" }
export interface Profile {
  name: string;
  emphasize: readonly string[];   // open lens hint (spec: freeform), e.g. multiplicity, aggregation/composition diamonds, generalization, realization
  hide: readonly ("operations" | "visibility")[];
  stereotypes: Record<string, StereotypeStyle>;
  palette: { metaclasses: readonly string[]; stereotypes: readonly string[] };
}

const PROFILES: Record<string, Profile> = { [UML_DOMAIN.name]: UML_DOMAIN };

/** Unknown or missing profile name falls back to uml-domain — never errors. */
export function getProfile(name?: string): Profile {
  return (name && PROFILES[name]) || UML_DOMAIN;
}

/** Merge every named stereotype's style; later stereotypes win per property. */
export function stereotypeStyle(profile: Profile, stereotypes: string[]): StereotypeStyle {
  return stereotypes.reduce<StereotypeStyle>((acc, s) => ({ ...acc, ...profile.stereotypes[s] }), {});
}
