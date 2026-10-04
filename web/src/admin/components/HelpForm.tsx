import { useState } from "react";
import { graphql, useMutation } from "react-relay";
import type { HelpFormQuery } from "./__generated__/HelpFormQuery.graphql";
import type { HelpFormMutation } from "./__generated__/HelpFormMutation.graphql";
import { useNotify } from "./useNotify";
import TextInput from "../../components/ui/TextInput";
import Textarea from "../../components/ui/Textarea";
import { Button } from "../../components/ui/Button";
import { useRetryableLazyLoadQuery } from "../../components/useRetryableLazyLoadQuery";
import { EmailHelp, HelpUses } from "../../components/EmailHelp";

// Matches the server's limits in `validate_feedback` (api/src/graphql/mutations.rs).
const SUBJECT_MAX = 200;
const MESSAGE_MAX = 10000;

interface HelpFormProps {
  /** Called with the ticket reference once the message is sent. */
  onSent: (reference: string) => void;
  /** Shown as a Cancel/Close button when given (the footer dialog). */
  onCancel?: () => void;
}

/**
 * Send a message to the seslogin team. The server turns it into a Toolbox
 * ticket from the signed-in user's own email address, so replies arrive in
 * their inbox. When that isn't configured on this server, it offers the
 * support email address instead. Used by the Help page and the admin footer's
 * "Help/Feedback?" link.
 */
export default function HelpForm({ onSent, onCancel }: HelpFormProps) {
  const data = useRetryableLazyLoadQuery<HelpFormQuery>(
    graphql`
      query HelpFormQuery @throwOnFieldError {
        feedbackAvailable
        user {
          email
        }
      }
    `,
    {},
  );

  const [commitMutation, isMutationInFlight] = useMutation<HelpFormMutation>(
    graphql`
      mutation HelpFormMutation($subject: String!, $message: String!) {
        submitFeedback(subject: $subject, message: $message) {
          number
          reference
        }
      }
    `,
  );

  const { notifyError, notifySuccess } = useNotify();
  // Remounting the form (new key) clears it after a successful send.
  const [formKey, setFormKey] = useState(0);

  async function handleSubmit(formData: FormData) {
    const subject = formData.get("subject")?.toString() ?? "";
    const message = formData.get("message")?.toString() ?? "";
    try {
      const reference = await new Promise<string>((resolve, reject) => {
        commitMutation({
          variables: { subject, message },
          onCompleted: (response) => resolve(response.submitFeedback.reference),
          onError: reject,
        });
      });
      setFormKey((k) => k + 1);
      notifySuccess(`Message sent — your reference is ${reference}`);
      onSent(reference);
    } catch (err) {
      notifyError(err, "Couldn't send your message");
    }
  }

  // Two complete variants rather than one with a caveat: someone who can't
  // use the form never sees it, or copy suggesting they can.
  if (!data.feedbackAvailable) {
    return <EmailHelp onClose={onCancel} />;
  }

  return (
    <div className="text-left">
      <p className="my-4">
        Send the seslogin team a message about anything, for example to:
      </p>
      <HelpUses />
      <p className="my-4">
        We&apos;ll reply by email to <strong>{data.user.email}</strong>.
      </p>
      <form key={formKey} action={handleSubmit} className="flex flex-col gap-4">
        <div className="flex flex-col gap-1">
          <label htmlFor="help-subject" className="font-title">
            Subject
          </label>
          <TextInput
            type="text"
            name="subject"
            id="help-subject"
            required
            maxLength={SUBJECT_MAX}
          />
        </div>
        <div className="flex flex-col gap-1">
          <label htmlFor="help-message" className="font-title">
            Message
          </label>
          <Textarea
            name="message"
            id="help-message"
            rows={8}
            required
            maxLength={MESSAGE_MAX}
          />
        </div>
        <div className="flex gap-3">
          <Button type="submit" disabled={isMutationInFlight}>
            {isMutationInFlight ? "Sending…" : "Send"}
          </Button>
          {onCancel && (
            <Button
              variant="secondary"
              onClick={onCancel}
              disabled={isMutationInFlight}
            >
              Cancel
            </Button>
          )}
        </div>
      </form>
    </div>
  );
}
