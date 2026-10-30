import type { ReactNode } from "react";
import { TextLink } from "../components/ui/TextLink";
import { SUPPORT_EMAIL, SUPPORT_MAILTO } from "../lib/support";

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section>
      <h2 className="mt-8 mb-2 font-title text-2xl font-bold">{title}</h2>
      {children}
    </section>
  );
}

/**
 * Public privacy policy: the privacy policy URL given to the App Store and
 * Google Play for Tally, and linked from inside the app. Apple (guideline
 * 5.1.1(i)) requires it to say what is collected and how it is used, who it
 * is shared with, how long it is kept, and how to ask for it to be deleted —
 * keep all four accurate when the data changes.
 */
export default function Privacy() {
  return (
    <div className="mx-auto max-w-150 px-4 py-10">
      <h1 className="font-title text-4xl font-bold text-accent">
        Privacy policy
      </h1>
      <p className="my-4 text-ink-muted">Last updated 30 October 2026</p>
      <p className="my-4">
        SESLogin records attendance for NSW SES units and gives members tools
        for their unit through the Tally app. This policy covers the SESLogin
        website (seslogin.com), its kiosks and the Tally app. Questions about it
        go to <TextLink href={SUPPORT_MAILTO}>{SUPPORT_EMAIL}</TextLink>.
      </p>

      <Section title="What we collect">
        <ul className="my-4 list-disc space-y-2 pl-6">
          <li>
            <strong>Your member details</strong>, from NSW SES&apos;s own
            systems rather than from you: name, preferred name, member number,
            unit, email address, phone numbers, capabilities and unit
            assignments.
          </li>
          <li>
            <strong>Attendance</strong>: when you sign in and out, where, and
            the activity you chose.
          </li>
          <li>
            <strong>What you add in Tally</strong>: tasks, notes, calendar
            events, map pins, equipment records and reservations, and on-call
            roles, each recorded with your name.
          </li>
          <li>
            <strong>Your location, only while you share it.</strong> When you
            start sharing in Tally&apos;s Locate, the app sends your position
            for the time you choose (up to an hour), including while the app is
            in the background. Showing where you are on the map otherwise stays
            on your phone.
          </li>
          <li>
            <strong>Signing in</strong>: the email address you enter and the
            one-time code we send it. Unit administrators also have their email
            address and any passkeys they add.
          </li>
          <li>
            <strong>A record of changes</strong>: who changed what and when, and
            the IP address it came from, so units can check their records.
          </li>
        </ul>
        <p className="my-4">
          We do not collect your phone&apos;s contacts, photos or advertising
          identifiers, and we do not use analytics, tracking or advertising.
        </p>
      </Section>

      <Section title="How we use it">
        <p className="my-4">
          Only to run SESLogin: recording attendance and producing your
          unit&apos;s reports, showing who is signed in, sending attendance to
          NSW SES&apos;s Beacon system, giving members a directory of NSW SES
          members and units, signing you in, and keeping the service secure. We
          never sell your information or use it for advertising.
        </p>
      </Section>

      <Section title="Who can see it">
        <ul className="my-4 list-disc space-y-2 pl-6">
          <li>
            Your unit&apos;s administrators see your attendance. Members at your
            unit see who is signed in and what you add for your unit.
          </li>
          <li>
            Any NSW SES member signed in to Tally can see the statewide member
            directory, which includes the member details above.
          </li>
          <li>
            A shared location is seen by members of the unit you share it with.
          </li>
          <li>NSW SES receives attendance through Beacon.</li>
          <li>
            Service providers that run parts of SESLogin for us: Amazon Web
            Services (hosting and email, with data stored in Australia), Ably
            (live kiosk updates, which carry names and sign-in times) and
            Cloudflare (checking sign-ins come from a person). Maps in Tally are
            drawn by Apple Maps or Google Maps under their own privacy policies.
            We share only what each needs to provide its service, and they
            protect it at least as well as this policy does.
          </li>
        </ul>
        <p className="my-4">
          We do not give your information to anyone else unless the law requires
          it.
        </p>
      </Section>

      <Section title="How long we keep it">
        <ul className="my-4 list-disc space-y-2 pl-6">
          <li>
            Member details and attendance are kept as your unit&apos;s records
            while SESLogin is in use. When you leave NSW SES, your member record
            is marked as deleted and no longer shown.
          </li>
          <li>
            Only your latest shared position is kept, and it is deleted
            automatically within minutes of your sharing ending.
          </li>
          <li>Notifications in Tally are deleted after 90 days.</li>
          <li>The record of changes is kept indefinitely.</li>
          <li>
            Tally keeps a copy of the directory on your phone so it works
            offline. Deleting the app removes it.
          </li>
        </ul>
      </Section>

      <Section title="Your choices">
        <ul className="my-4 list-disc space-y-2 pl-6">
          <li>
            You can stop sharing your location at any time, and turn off
            Tally&apos;s location access in your phone&apos;s settings.
          </li>
          <li>
            Your member details come from NSW SES, so ask your unit to correct
            them there and SESLogin will update.
          </li>
          <li>
            To see the information we hold about you, correct it, or ask us to
            delete it, email{" "}
            <TextLink href={SUPPORT_MAILTO}>{SUPPORT_EMAIL}</TextLink>. We will
            reply within 30 days. We delete anything that is not needed as your
            unit&apos;s attendance record and tell you what, if anything, we
            must keep and why.
          </li>
        </ul>
      </Section>

      <Section title="Security">
        <p className="my-4">
          Information is encrypted in transit and at rest, and each person sees
          only what their role at a unit allows.
        </p>
      </Section>

      <Section title="Changes">
        <p className="my-4">
          If this policy changes, we update this page and the date at the top.
        </p>
      </Section>

      <Section title="Contact">
        <p className="my-4">
          Email <TextLink href={SUPPORT_MAILTO}>{SUPPORT_EMAIL}</TextLink>, or
          see <TextLink to="/support">Support</TextLink>.
        </p>
      </Section>
    </div>
  );
}
