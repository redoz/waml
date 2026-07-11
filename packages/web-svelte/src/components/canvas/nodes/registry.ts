import type { Component } from "svelte";
import { splitType } from "@mc/okf";
import type { OkfNodeData } from "./types";
import GenericNode from "./GenericNode.svelte";
import UmlClassNode from "./UmlClassNode.svelte";
import UmlInterfaceNode from "./UmlInterfaceNode.svelte";
import UmlEnumNode from "./UmlEnumNode.svelte";
import UmlDataTypeNode from "./UmlDataTypeNode.svelte";
import UmlPackageNode from "./UmlPackageNode.svelte";
import UmlAssociationNode from "./UmlAssociationNode.svelte";
import UmlNoteNode from "./UmlNoteNode.svelte";

type NodeComponent = Component<{ data: OkfNodeData }>;

// Closed metaclass set per family — everything else degrades to GenericNode.
const FAMILIES: Record<string, Record<string, NodeComponent>> = {
  uml: {
    Class: UmlClassNode,
    Interface: UmlInterfaceNode,
    Enum: UmlEnumNode,
    DataType: UmlDataTypeNode,
    Package: UmlPackageNode,
    Association: UmlAssociationNode, // association class — class box + dashed mid-line connector (edge side)
    Note: UmlNoteNode, // dog-eared comment box — no compartments
  },
};

export function resolveNodeRenderer(type: string): NodeComponent {
  const t = splitType(type);
  return (t && FAMILIES[t.family]?.[t.metaclass]) ?? GenericNode;
}
