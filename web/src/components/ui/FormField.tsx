import type { ReactNode } from "react";

export function FieldList({ children }: { children: ReactNode }) {
  return (
    <dl className="mx-auto my-8 grid w-full max-w-[700px] grid-cols-1 border-y border-neutral-300 py-2.5 text-left md:grid-cols-[45%_1fr] md:text-right">
      {children}
    </dl>
  );
}

export function FormField({
  label,
  children,
}: {
  label?: ReactNode;
  children?: ReactNode;
}) {
  if (!label && !children) {
    // Legacy spacer row (`<dt>&nbsp;</dt>`): collapses on mobile.
    return <dt className="max-md:hidden" />;
  }
  return (
    <>
      <dt className="mt-3.5 mr-2.5 font-title md:text-right">{label}</dt>
      <dd className="my-2.5 min-h-[22px] text-left md:ml-2.5">{children}</dd>
    </>
  );
}
