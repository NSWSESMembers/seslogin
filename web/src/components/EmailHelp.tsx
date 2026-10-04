import { Button } from "./ui/Button";
import { buttonSizes, buttonVariants } from "./ui/buttonStyles";
import { cn } from "../lib/tw";
import { SUPPORT_EMAIL, SUPPORT_MAILTO } from "../lib/support";

/** What the help/feedback channel is for, shared by every variant of it. */
export function HelpUses() {
  return (
    <ul className="my-4 list-disc pl-6">
      <li>ask for help using seslogin</li>
      <li>report a bug or something that doesn&apos;t look right</li>
      <li>get a new user set up</li>
      <li>request a new feature or suggest an improvement</li>
    </ul>
  );
}

/**
 * Help by email: the whole content for anyone who can't file a ticket
 * directly — the public home page, or an admin site with Toolbox not
 * configured. Deliberately no form and no mention of one.
 */
export function EmailHelp({ onClose }: { onClose?: () => void }) {
  return (
    <div className="text-left">
      <p className="my-4">
        Email the seslogin team at <a href={SUPPORT_MAILTO}>{SUPPORT_EMAIL}</a>{" "}
        about anything, for example to:
      </p>
      <HelpUses />
      <div className="flex gap-3">
        <a
          href={SUPPORT_MAILTO}
          className={cn(
            "inline-block cursor-pointer align-middle no-underline",
            buttonVariants.primary,
            buttonSizes.normal,
          )}
        >
          Write an email
        </a>
        {onClose && (
          <Button variant="secondary" onClick={onClose}>
            Close
          </Button>
        )}
      </div>
    </div>
  );
}
