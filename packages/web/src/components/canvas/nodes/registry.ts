import type { ComponentType } from "react";
import { splitType } from "@mc/okf";
import { GenericNode } from "./GenericNode";
import { UmlClassNode, UmlInterfaceNode, UmlEnumNode, UmlDataTypeNode, UmlPackageNode, UmlAssociationNode, UmlNoteNode } from "./uml";
import type { OkfNodeProps } from "./shared";

// Closed metaclass set per family — everything else degrades to GenericNode.
const FAMILIES: Record<string, Record<string, ComponentType<OkfNodeProps>>> = {
  uml: {
    Class: UmlClassNode,
    Interface: UmlInterfaceNode,
    Enum: UmlEnumNode,
    DataType: UmlDataTypeNode,
    Package: UmlPackageNode,
    Association: UmlAssociationNode,   // association class — class box + dashed mid-line connector (edge side)
    Note: UmlNoteNode,                 // dog-eared comment box — no compartments
  },
};

export function resolveNodeRenderer(type: string): ComponentType<OkfNodeProps> {
  const t = splitType(type);
  return (t && FAMILIES[t.family]?.[t.metaclass]) ?? GenericNode;
}
