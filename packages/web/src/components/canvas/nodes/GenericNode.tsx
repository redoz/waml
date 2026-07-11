import { ClassifierBox, type OkfNodeProps } from "./shared";

export function GenericNode({ data }: OkfNodeProps) {
  return (
    <ClassifierBox data={data}
      header={<div className="px-3 pt-[8px]">
        <span className="text-[10px] font-[650] uppercase tracking-[0.3px] px-[7px] py-[2px] rounded-full text-white bg-[#94a3b8]">
          {data.type}
        </span>
      </div>} />
  );
}
