import type { Profile } from "./index";

export const UML_DOMAIN: Profile = {
  name: "uml-domain",
  emphasize: ["multiplicity", "aggregation", "composition", "generalization", "realization"],
  hide: ["operations", "visibility"],
  stereotypes: {
    aggregateRoot: { header: "#eab308", border: "thick" }, // gold
    valueObject: { header: "#64748b" },                    // slate
    domainEvent: { shape: "hexagon" },
  },
  palette: {
    // `uml.Association` and `uml.Note` are intentionally omitted: association classes
    // are authored via an `as [link]` name on a relationship, and notes via the `## Notes`
    // shorthand / a standalone note doc — never by adding a bare node from the palette.
    // Both still render (Task 11) when present in an imported/authored model.
    metaclasses: ["uml.Class", "uml.Interface", "uml.Enum", "uml.DataType"],
    stereotypes: ["entity", "valueObject", "aggregateRoot", "service", "domainEvent"],
  },
};
