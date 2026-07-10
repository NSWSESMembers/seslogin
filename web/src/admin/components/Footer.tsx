import GraphiQLLink from "../../components/GraphiQLLink";
import ClientVersionLabel from "../../components/ClientVersionLabel";

export default function Footer() {
  return (
    <footer className="bg-surface-sunken p-2.5 text-xs text-ink-muted">
      NSW SES Volunteers &mdash; SES Activity v2 &mdash; <ClientVersionLabel />
      &mdash; <GraphiQLLink />
    </footer>
  );
}
