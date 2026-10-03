import { useState, type ReactNode } from "react";
import { Dialog, DialogTitle } from "./ui/Dialog";
import { SUPPORT_MAILTO } from "../lib/support";

/**
 * The footer's "Help/Feedback?" link: opens a dialog whose content `children`
 * renders, given a close callback. The href stays a real mailto so copying
 * the link or opening it in a new tab still works.
 */
export default function HelpLink({
  children,
}: {
  children: (close: () => void) => ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const close = () => setOpen(false);

  return (
    <>
      <a
        href={SUPPORT_MAILTO}
        onClick={(event) => {
          event.preventDefault();
          setOpen(true);
        }}
      >
        Help/Feedback?
      </a>
      {open && (
        <Dialog onDismiss={close}>
          <DialogTitle>Help and feedback</DialogTitle>
          {children(close)}
        </Dialog>
      )}
    </>
  );
}
