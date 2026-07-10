import type { ReactNode } from "react";

export function Panel({ children }: { children: ReactNode }) {
  return (
    <section className="flex min-h-screen items-center justify-center bg-[radial-gradient(circle_at_top,rgba(255,205,161,0.65),transparent_32%),linear-gradient(180deg,#fff3e7_0%,#f7ede4_45%,#efe3d8_100%)] px-6 py-12">
      {children}
    </section>
  );
}

export function PanelBox({ children }: { children: ReactNode }) {
  return (
    <div className="w-[min(640px,100%)] rounded-[28px] border border-[rgba(139,75,36,0.18)] bg-[rgba(255,252,248,0.94)] p-10 text-left shadow-[0_24px_80px_rgba(101,53,30,0.16)]">
      {children}
    </div>
  );
}

export function PanelTitle({ children }: { children: ReactNode }) {
  return (
    <h1 className="m-0 font-title text-4xl/none text-navy md:text-5xl/none">
      {children}
    </h1>
  );
}

export function PanelIntro({ children }: { children: ReactNode }) {
  return (
    <p className="mt-4 mb-6 max-w-136 text-xl/normal text-[#40342d]">
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
      ? "border-submenu/25 bg-brand/15 text-[#7a350f]"
      : "border-red-900/20 bg-red-900/10 text-red-900";
  return (
    <div
      className={`mb-4 rounded-2xl border px-4 py-3.5 leading-snug ${variantClasses}`}
    >
      {children}
    </div>
  );
}
