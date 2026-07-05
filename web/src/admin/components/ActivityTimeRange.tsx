import TextInput from "../../components/ui/TextInput";

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
      <label>
        Start time:&nbsp;
        <TextInput
          type="datetime-local"
          value={startInput}
          onChange={(e) => onStartChange(e.target.value)}
        />
      </label>
      <label>
        End time:&nbsp;
        <TextInput
          type="datetime-local"
          value={endInput}
          onChange={(e) => onEndChange(e.target.value)}
        />
      </label>
    </div>
  );
}
