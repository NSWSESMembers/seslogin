import { Link } from "react-router";
import { EmailHelp } from "../components/EmailHelp";

/**
 * Public support page: the Support URL given to the App Store and Google Play
 * for Tally, so it must name a real way to reach us without signing in.
 */
export default function Support() {
  return (
    <div className="mx-auto max-w-150 px-4 py-10">
      <h1 className="font-title text-4xl font-bold text-accent">Support</h1>
      <p className="my-4">
        Help with SESLogin and the Tally app is available by email.
      </p>
      <EmailHelp />
      <p className="my-4">
        If something isn&apos;t working, tell us what you were doing, what
        happened, and which device you were using, so we can look into it.
      </p>
      <p className="my-4">
        For step-by-step instructions, see the{" "}
        <Link to="/guide">SESLogin user guides</Link>.
      </p>
    </div>
  );
}
