import type { ReactNode } from "react";
import {
  Panel,
  PanelBox,
  PanelTitle,
  PanelIntro,
} from "../../components/ui/Panel";

const STEPS: ReactNode[] = [
  <>
    Tap the <strong>Share</strong> button (the square with an arrow pointing up)
    in the browser's toolbar.
  </>,
  <>
    Choose <strong>Add to Home Screen</strong>, then tap <strong>Add</strong>.
  </>,
  <>
    Close this tab, open <strong>seslogin</strong> from the new Home Screen
    icon, and enroll the kiosk there.
  </>,
];

/**
 * Shown on an iPad's enrollment screen in place of the QR code, while the kiosk is
 * still running in a browser tab. A Home Screen app runs without browser chrome and
 * can hold the wake lock, which a tab can't reliably do on older iPadOS — and it keeps
 * its own storage, so enrolling the tab first would only have to be done again. Hence
 * catching it here, before anyone spends an enrollment on the wrong one.
 */
export default function KioskHomeScreenHelp({
  onIgnore,
}: {
  onIgnore: () => void;
}) {
  return (
    <Panel>
      <PanelBox>
        <PanelTitle>Add this kiosk to the Home Screen</PanelTitle>
        <PanelIntro>
          On an iPad, the kiosk works best as a Home Screen app: it fills the
          whole screen, without the browser's toolbars, and stays awake.
        </PanelIntro>

        <ol className="mb-6 flex list-none flex-col gap-4 p-0">
          {STEPS.map((step, index) => (
            <li key={index} className="flex items-start gap-3">
              <span className="flex size-6 shrink-0 items-center justify-center rounded-full bg-accent text-sm font-bold text-white">
                {index + 1}
              </span>
              <p className="m-0 text-sm text-ink">{step}</p>
            </li>
          ))}
        </ol>

        <p className="mb-6 text-sm text-ink opacity-70">
          The Home Screen app keeps its own storage, so a kiosk enrolled in this
          tab would need enrolling again. Once it's set up, Guided Access
          (Settings &gt; Accessibility) stops anyone leaving the kiosk.
        </p>

        <p className="m-0 text-center">
          <button
            type="button"
            onClick={onIgnore}
            className="cursor-pointer border-0 bg-transparent p-0 text-xs text-ink underline opacity-60"
          >
            Ignore
          </button>
        </p>
      </PanelBox>
    </Panel>
  );
}
