import { Link } from "react-router";
import logo from "../assets/logo.svg";

export default function Home() {
  return (
    <div className="bg-white">
      <div className="bg-white">
        <img className="mx-auto box-content pt-[60px] pb-5" src={logo} alt="" />

        <p className="mx-4 my-10 text-2xl md:mx-10">
          Welcome, please choose an option to continue...
        </p>

        <ul className="m-0 mb-12 list-none p-0 text-lg font-bold">
          <li className="text-left">
            <Link
              to="/kiosk"
              className="group mx-auto flex w-full max-w-150 items-center gap-4 border-t border-gray-500 px-5 py-1.25 text-black no-underline md:min-h-16.25"
            >
              <span className="w-25 shrink-0 font-title text-4xl font-bold text-accent group-hover:text-accent-light">
                Kiosk
              </span>
              <p className="flex-1 text-navy group-hover:text-[#6c81c1]">
                Allow members to sign in and out using this computer
              </p>
              <img
                src="/image/home-icons/scan.png"
                alt="Kiosk"
                className="box-content h-16.25 w-16.25 shrink-0 max-md:hidden"
              />
            </Link>
          </li>

          <li className="text-left">
            <Link
              to="/admin"
              className="group mx-auto flex w-full max-w-150 items-center gap-4 border-t border-b border-gray-500 px-5 py-1.25 text-black no-underline md:min-h-16.25"
            >
              <span className="w-25 shrink-0 font-title text-4xl font-bold text-accent group-hover:text-accent-light">
                Admin
              </span>
              <p className="flex-1 text-navy group-hover:text-[#6c81c1]">
                Use the administrator dashboard to administer your unit, create
                reports and view activity
              </p>
              <img
                src="/image/home-icons/dash.png"
                alt="Admin"
                className="box-content h-16.25 w-16.25 shrink-0 max-md:hidden"
              />
            </Link>
          </li>
        </ul>
      </div>
    </div>
  );
}
