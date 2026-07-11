import { ClassifierBox, NodePorts, type OkfNodeProps } from "./shared";

export function UmlClassNode({ data }: OkfNodeProps) {
  return <ClassifierBox data={data} />;
}
export function UmlInterfaceNode({ data }: OkfNodeProps) {
  return <ClassifierBox data={data} keyword="interface" />;
}
export function UmlEnumNode({ data }: OkfNodeProps) {
  return <ClassifierBox data={data} keyword="enumeration" />;
}
export function UmlDataTypeNode({ data }: OkfNodeProps) {
  return <ClassifierBox data={data} keyword="dataType" />;
}
export function UmlPackageNode({ data }: OkfNodeProps) {
  // Tabbed-folder: a small tab above the box.
  return (
    <div className="relative">
      <div className="absolute -top-[10px] left-[10px] h-[12px] w-[64px] rounded-t-md border-[1.5px] border-b-0 border-[#d8dee8] bg-white" />
      <ClassifierBox data={data} />
    </div>
  );
}
export function UmlAssociationNode({ data }: OkfNodeProps) {
  // Association class: an ordinary class box (name / attributes), drawn with a dashed
  // outline. The dashed connector from the box to the association line's midpoint is
  // drawn by the edge renderer (Task 12) for the edge whose `name = { ref }` points here.
  return <div className="[&>div]:border-dashed"><ClassifierBox data={data} keyword="association" /></div>;
}
export function UmlNoteNode({ data }: OkfNodeProps) {
  // UML Comment: a dog-eared note box carrying the markdown body; NO attribute /
  // operation compartments. Its dashed anchor(s) to the annotated element(s) are
  // drawn by the edge/anchor layer (Task 12).
  return (
    <div className="relative w-[210px] bg-[#fffdf3] border-[1.5px] border-[#e3d9a8] shadow-[0_2px_8px_rgba(15,23,42,0.05)] select-none"
      style={{ clipPath: "polygon(0 0, calc(100% - 14px) 0, 100% 14px, 100% 100%, 0 100%)" }}>
      <div className="absolute top-0 right-0 h-[14px] w-[14px] border-l border-b border-[#e3d9a8] bg-[#f3ebc0]" />
      <div className="px-3 py-[9px] text-[11.5px] leading-snug text-slate-700 whitespace-pre-wrap">
        {data.body ?? data.title}
      </div>
      <NodePorts />
    </div>
  );
}
