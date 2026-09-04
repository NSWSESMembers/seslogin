import { Link } from "react-router";
import logo from "../assets/logo.svg";
import { AdminIcon, KioskIcon } from "./HomeIcons";

export default function Home() {
  return (
    <div className="bg-surface">
      <div className="bg-surface">
        <img className="mx-auto box-content pt-15 pb-5" src={logo} alt="" />

        <p className="mx-4 my-10 text-2xl md:mx-10">
          Welcome, please choose an option to continue...
        </p>

        <ul className="m-0 mb-12 list-none p-0 text-lg font-bold">
          <li className="text-left">
            <Link
              to="/kiosk"
              className="group mx-auto flex w-full max-w-150 items-center gap-4 border-t border-line-strong px-5 py-1.25 text-ink no-underline md:min-h-16.25"
            >
              <span className="shrink-0 font-title text-4xl font-bold whitespace-nowrap text-accent group-hover:text-accent-light">
                Kiosk
              </span>
              <p className="flex-1 text-navy group-hover:text-[#6c81c1]">
                Allow members to sign in and out using this computer
              </p>
              <KioskIcon className="size-16.25 shrink-0 text-navy group-hover:text-[#6c81c1] group-hover:[--icon-accent:var(--color-accent-light)] max-md:hidden" />
            </Link>
          </li>

          <li className="text-left">
            <Link
              to="/admin"
              className="group mx-auto flex w-full max-w-150 items-center gap-4 border-y border-line-strong px-5 py-1.25 text-ink no-underline md:min-h-16.25"
            >
              <span className="shrink-0 font-title text-4xl font-bold whitespace-nowrap text-accent group-hover:text-accent-light">
                Admin
              </span>
              <p className="flex-1 text-navy group-hover:text-[#6c81c1]">
                Use the administrator dashboard to administer your unit, create
                reports and view activity
              </p>
              <AdminIcon className="size-16.25 shrink-0 text-navy group-hover:text-[#6c81c1] group-hover:[--icon-accent:var(--color-accent-light)] max-md:hidden" />
            </Link>
          </li>
        </ul>
      </div>
    </div>
  );
}
