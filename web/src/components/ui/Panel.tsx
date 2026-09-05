import type { ReactNode } from "react";

export function Panel({ children }: { children: ReactNode }) {
  return (
    <section className="flex min-h-screen items-center justify-center bg-[rgba(255,252,248,0.94)] px-4 py-8 sm:bg-[radial-gradient(circle_at_top,rgba(255,205,161,0.65),transparent_32%),linear-gradient(180deg,#fff3e7_0%,#f7ede4_45%,#efe3d8_100%)] sm:px-6 sm:py-12 dark:bg-[rgba(32,30,28,0.94)] sm:dark:bg-[radial-gradient(circle_at_top,rgba(120,70,30,0.32),transparent_38%),linear-gradient(180deg,#1c1a18_0%,#161514_55%,#121110_100%)]">
      {children}
    </section>
  );
}

export function PanelBox({ children }: { children: ReactNode }) {
  return (
    <div className="w-[min(640px,100%)] rounded-none border-0 bg-[rgba(255,252,248,0.94)] p-6 text-left shadow-none sm:rounded-[28px] sm:border sm:border-[rgba(139,75,36,0.18)] sm:p-10 sm:shadow-[0_24px_80px_rgba(101,53,30,0.16)] dark:bg-[rgba(32,30,28,0.94)] sm:dark:border-white/10 sm:dark:shadow-[0_24px_80px_rgba(0,0,0,0.55)]">
      {children}
    </div>
  );
}

export function PanelTitle({ children }: { children: ReactNode }) {
  return (
    <h1 className="m-0 font-title text-3xl/none text-navy sm:text-4xl/none md:text-5xl/none">
      {children}
    </h1>
  );
}

export function PanelIntro({ children }: { children: ReactNode }) {
  return (
    <p className="mt-4 mb-6 max-w-136 text-lg/normal text-[#40342d] sm:text-xl/normal dark:text-ink">
      {children}
    </p>
  );
}

export function PanelMessage({
  variant = "error",
  children,
}: {
  variant?: "error" | "warning";
  children: ReactNode;
}) {
  const variantClasses =
    variant === "warning"
      ? "border-submenu/25 bg-brand/15 text-[#7a350f] dark:text-accent-light"
      : "border-red-900/20 bg-red-900/10 text-red-900 dark:border-red-400/25 dark:bg-red-500/15 dark:text-red-300";
  return (
    <div
      className={`mb-4 rounded-2xl border px-4 py-3.5 leading-snug ${variantClasses}`}
    >
      {children}
    </div>
  );
}
