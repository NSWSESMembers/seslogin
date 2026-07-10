import { RelayEnvironmentProvider } from "react-relay";
import { unauthenticatedEnvironment } from "../../lib/environments";
import KioskSetupForm from "../components/KioskSetupForm";

export default function KioskSetup({
  onEnrollWithQr,
}: {
  onEnrollWithQr: () => void;
}) {
  return (
    <RelayEnvironmentProvider environment={unauthenticatedEnvironment}>
      <KioskSetupForm onEnrollWithQr={onEnrollWithQr} />
    </RelayEnvironmentProvider>
  );
}
