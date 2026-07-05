import GraphiQLLink from "../../components/GraphiQLLink";
import ClientVersionLabel from "../../components/ClientVersionLabel";

export default function Footer() {
  return (
    <footer className="bg-[#dddddd] p-2.5 text-[12px] text-[#333333]">
      NSW SES Volunteers &mdash; SES Activity v2 &mdash; <ClientVersionLabel />
      &mdash; <GraphiQLLink />
    </footer>
  );
}
