import { useState } from "react";
import { SectionHeading } from "../../components/ui/SectionHeading";
import HelpForm from "../components/HelpForm";

export default function Help() {
  const [sentReference, setSentReference] = useState<string | null>(null);
  return (
    <div className="mx-auto max-w-175">
      <SectionHeading>Help and feedback</SectionHeading>
      {sentReference && (
        <p className="my-4" role="status">
          Thanks — your message was sent. Replies will arrive by email with{" "}
          <strong>{sentReference}</strong> in the subject.
        </p>
      )}
      <HelpForm onSent={setSentReference} />
    </div>
  );
}
