import { ButtonLink } from "../../components/ui/Button";

type SessionCreatedInterstitialProps = {
  code: string;
};

export default function SessionCreatedInterstitial({
  code,
}: SessionCreatedInterstitialProps) {
  return (
    <div>
      <p>
        Your kiosk session has been created and can now be assigned to a
        computer.
      </p>
      <p>
        Please enter the following code on your kiosk computer by accessing the{" "}
        <b>SES Activity</b> (seslogin.com) system and selecting the <b>Kiosk</b>{" "}
        module.
      </p>

      <p className="my-6 text-center text-6xl font-bold tracking-[0.08em]">
        {code || "Unavailable"}
      </p>

      <p className="text-center">
        <ButtonLink to="/admin/sessions">Continue</ButtonLink>
      </p>
    </div>
  );
}
