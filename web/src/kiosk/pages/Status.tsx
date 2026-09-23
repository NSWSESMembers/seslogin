import { Button } from "../../components/ui/Button";
import { StatusMessage } from "../../components/ui/StatusMessage";
import StatusCurrentDisplay from "../components/StatusCurrentDisplay";
import { useLivePeriods } from "../components/useLivePeriods";

export default function Status() {
  const { periods, loading, error, retry } = useLivePeriods();

  if (loading) {
    return (
      <div className="flex h-dvh items-center justify-center">
        <p className="text-ink-muted">Loading…</p>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex h-dvh flex-col items-center justify-center gap-4 p-6 text-center">
        <StatusMessage variant="error">
          Couldn't load the signed-in list: {error}
        </StatusMessage>
        <Button onClick={retry}>Try again</Button>
      </div>
    );
  }

  return <StatusCurrentDisplay periods={periods} />;
}
