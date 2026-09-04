import { useState } from "react";
import { Button } from "../../components/ui/Button";

// Shows a one-time secret (e.g. a freshly created API token) in a
// monospace box with a copy-to-clipboard button.
export default function CopyableSecret({ secret }: { secret: string }) {
  const [copied, setCopied] = useState(false);

  async function copy() {
    try {
      await navigator.clipboard.writeText(secret);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // Clipboard access can be denied (permissions, insecure context) —
      // the secret is still selectable and copyable by hand.
    }
  }

  return (
    <div className="flex flex-col gap-2 rounded-lg border border-line bg-surface-raised p-3.5 md:flex-row md:items-center">
      <code className="min-w-0 flex-1 text-sm wrap-break-word">{secret}</code>
      <Button type="button" size="row" onClick={copy}>
        {copied ? "Copied!" : "Copy"}
      </Button>
    </div>
  );
}
