import TimeInputWithControls from "../../components/ui/TimeInputWithControls";

type Props = {
  startInput: string;
  endInput: string;
  onStartChange: (value: string) => void;
  onEndChange: (value: string) => void;
};

export default function ActivityTimeRange({
  startInput,
  endInput,
  onStartChange,
  onEndChange,
}: Props) {
  return (
    <div className="mb-4 flex justify-center gap-5 max-md:flex-col max-md:items-center">
      <label className="flex items-center gap-2">
        Start time:&nbsp;
        <TimeInputWithControls value={startInput} onChange={onStartChange} />
      </label>
      <label className="flex items-center gap-2">
        End time:&nbsp;
        <TimeInputWithControls
          value={endInput}
          onChange={onEndChange}
          copyFrom={{ label: "Copy start time", value: startInput }}
        />
      </label>
    </div>
  );
}
