import { useState, type ChangeEvent } from "react";
import { FieldList, FormField } from "../../components/ui/FormField";
import TextInput from "../../components/ui/TextInput";
import { Button } from "../../components/ui/Button";

interface SessionFormProps {
  initialName: string;
  initialConfig: string;
  initialHealthcheckUrl: string;
  isMutationInFlight: boolean;
  onSubmit: (formData: FormData) => void | Promise<void>;
}

type ConfigEditorMode = "basic" | "advanced";
type SessionMode = "scan" | "status";
type ConfigObject = Record<string, unknown>;

interface ConfigEditorModeControlProps {
  configEditorMode: ConfigEditorMode;
  onSetEditorMode: (nextEditorMode: ConfigEditorMode) => void;
}

interface BasicSessionModeFieldsProps {
  sessionMode: SessionMode;
  onChange: (nextMode: SessionMode) => void;
  smallCategories: boolean;
  onSmallCategoriesChange: (next: boolean) => void;
  configJson: string;
}

interface AdvancedConfigFieldsProps {
  configJson: string;
  onChange: (event: ChangeEvent<HTMLTextAreaElement>) => void;
}

interface SubmitRowProps {
  isMutationInFlight: boolean;
}

interface InitialConfigState {
  normalizedConfigJson: string;
}

function parseConfigObject(configText: string): ConfigObject {
  try {
    const parsed = JSON.parse(configText);
    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
      return parsed as ConfigObject;
    }
  } catch {
    // Ignore parse errors and fall back to empty object.
  }

  return {};
}

function withSessionMode(
  config: ConfigObject,
  sessionMode: SessionMode,
): ConfigObject {
  const next = { ...config };
  if (sessionMode === "status") {
    next.status = true;
  } else {
    delete next.status;
  }
  return next;
}

function getSessionModeFromConfig(config: ConfigObject): SessionMode {
  return config.status ? "status" : "scan";
}

function withSmallCategories(
  config: ConfigObject,
  enabled: boolean,
): ConfigObject {
  const next = { ...config };
  if (enabled) {
    next.smallCategories = true;
  } else {
    delete next.smallCategories;
  }
  return next;
}

function getSmallCategoriesFromConfig(config: ConfigObject): boolean {
  return !!config.smallCategories;
}

function initializeConfigState(initialConfig: string): InitialConfigState {
  const parsed = parseConfigObject(initialConfig);
  const sessionMode = getSessionModeFromConfig(parsed);
  const normalizedConfig = withSessionMode(parsed, sessionMode);

  return {
    normalizedConfigJson: JSON.stringify(normalizedConfig, null, 2),
  };
}

function NameField({ initialName }: { initialName: string }) {
  return (
    <FormField label={<label htmlFor="name">Name</label>}>
      <TextInput
        type="text"
        name="name"
        id="name"
        defaultValue={initialName}
        required
      />
    </FormField>
  );
}

function ConfigEditorModeControl({
  configEditorMode,
  onSetEditorMode,
}: ConfigEditorModeControlProps) {
  return (
    <FormField label={<span>Config Editor</span>}>
      <div
        className="inline-flex overflow-hidden rounded-lg border border-neutral-400"
        role="group"
        aria-label="Config editor mode"
      >
        <button
          className="m-0 min-w-23 cursor-pointer rounded-none border-0 bg-neutral-100 px-3 py-1.5 text-neutral-800 hover:bg-neutral-200 aria-pressed:bg-navy aria-pressed:text-white aria-pressed:hover:bg-[#2b4f97]"
          type="button"
          onClick={() => onSetEditorMode("basic")}
          aria-pressed={configEditorMode === "basic"}
        >
          Basic
        </button>
        <button
          className="m-0 min-w-23 cursor-pointer rounded-none border-0 border-l border-neutral-400 bg-neutral-100 px-3 py-1.5 text-neutral-800 hover:bg-neutral-200 aria-pressed:bg-navy aria-pressed:text-white aria-pressed:hover:bg-[#2b4f97]"
          type="button"
          onClick={() => onSetEditorMode("advanced")}
          aria-pressed={configEditorMode === "advanced"}
        >
          Advanced
        </button>
      </div>
    </FormField>
  );
}

function BasicSessionModeFields({
  sessionMode,
  onChange,
  smallCategories,
  onSmallCategoriesChange,
  configJson,
}: BasicSessionModeFieldsProps) {
  return (
    <>
      <FormField label={<span>Mode</span>}>
        <div className="grid gap-2" role="radiogroup" aria-label="Mode">
          <label className="grid grid-cols-[auto_auto_1fr] items-start gap-x-2">
            <input
              type="radio"
              name="sessionMode"
              value="scan"
              checked={sessionMode === "scan"}
              onChange={() => onChange("scan")}
              className="mt-1"
            />
            <span className="font-semibold">Scan</span>
            <span className="text-neutral-600">
              allow members to sign in and out on this computer (touchscreen or
              mouse and keyboard required)
            </span>
          </label>
          <label className="grid grid-cols-[auto_auto_1fr] items-start gap-x-2">
            <input
              type="radio"
              name="sessionMode"
              value="status"
              checked={sessionMode === "status"}
              onChange={() => onChange("status")}
              className="mt-1"
            />
            <span className="font-semibold">Status</span>
            <span className="text-neutral-600">
              show a live-updating non-interactive list of who is currently
              signed in at the unit along with how long they've been signed in
              for
            </span>
          </label>
        </div>
        <input type="hidden" name="config" value={configJson} />
      </FormField>
      {sessionMode === "scan" && (
        <FormField label={<span>Options</span>}>
          <label className="grid grid-cols-[auto_auto_1fr] items-start gap-x-2">
            <input
              type="checkbox"
              checked={smallCategories}
              onChange={(e) => onSmallCategoriesChange(e.target.checked)}
              className="mt-1"
            />
            <span className="font-semibold">Small categories</span>
            <span className="text-neutral-600">
              use smaller category buttons to fit more on screen — useful on
              smaller or lower-resolution displays
            </span>
          </label>
        </FormField>
      )}
    </>
  );
}

function HealthcheckUrlField({
  initialHealthcheckUrl,
}: {
  initialHealthcheckUrl: string;
}) {
  return (
    <FormField label={<label htmlFor="healthcheckUrl">Health Check URL</label>}>
      <TextInput
        type="url"
        name="healthcheckUrl"
        id="healthcheckUrl"
        defaultValue={initialHealthcheckUrl}
        placeholder="https://hc-ping.com/..."
        autoCapitalize="none"
        autoCorrect="off"
        autoComplete="url"
        inputMode="url"
      />
      <p className="mt-1.5 mb-0 text-neutral-600">
        Optional. SES Activity can ping this URL approximately every 5 minutes
        or so while the kiosk using this session remains connected to the
        system. Perfect for use with something like{" "}
        <a
          href="https://healthchecks.io/"
          target="_blank"
          rel="noreferrer"
          className="underline"
        >
          healthchecks.io
        </a>{" "}
        to automatically notify you when the kiosk isn't working.
      </p>
    </FormField>
  );
}

function AdvancedConfigFields({
  configJson,
  onChange,
}: AdvancedConfigFieldsProps) {
  return (
    <FormField label={<label htmlFor="config">Config (JSON object)</label>}>
      <textarea
        name="config"
        id="config"
        rows={8}
        value={configJson}
        onChange={onChange}
        spellCheck={false}
        className="w-full rounded-md border border-neutral-300 p-2 font-mono text-sm"
      />
    </FormField>
  );
}

function SubmitRow({ isMutationInFlight }: SubmitRowProps) {
  return (
    <FormField>
      <Button type="submit" disabled={isMutationInFlight}>
        Save
      </Button>
    </FormField>
  );
}

export default function SessionForm({
  initialName,
  initialConfig,
  initialHealthcheckUrl,
  isMutationInFlight,
  onSubmit,
}: SessionFormProps) {
  const initialState = initializeConfigState(initialConfig);
  const [configEditorMode, setConfigEditorMode] =
    useState<ConfigEditorMode>("basic");
  const [configJson, setConfigJson] = useState<string>(
    initialState.normalizedConfigJson,
  );
  const parsedConfig = parseConfigObject(configJson);
  const sessionMode = getSessionModeFromConfig(parsedConfig);
  const smallCategories = getSmallCategoriesFromConfig(parsedConfig);

  function setEditorMode(nextEditorMode: ConfigEditorMode) {
    if (configEditorMode === nextEditorMode) {
      return;
    }
    setConfigEditorMode(nextEditorMode);
  }

  function handleBasicSessionModeChange(nextMode: SessionMode) {
    const nextConfig = withSessionMode(parseConfigObject(configJson), nextMode);
    setConfigJson(JSON.stringify(nextConfig, null, 2));
  }

  function handleSmallCategoriesChange(enabled: boolean) {
    const nextConfig = withSmallCategories(
      parseConfigObject(configJson),
      enabled,
    );
    setConfigJson(JSON.stringify(nextConfig, null, 2));
  }

  function handleAdvancedConfigChange(event: ChangeEvent<HTMLTextAreaElement>) {
    const nextConfigText = event.target.value;
    setConfigJson(nextConfigText);
  }

  return (
    <form action={onSubmit}>
      <FieldList>
        <NameField initialName={initialName} />
        <ConfigEditorModeControl
          configEditorMode={configEditorMode}
          onSetEditorMode={setEditorMode}
        />
        {configEditorMode === "basic" && (
          <BasicSessionModeFields
            sessionMode={sessionMode}
            onChange={handleBasicSessionModeChange}
            smallCategories={smallCategories}
            onSmallCategoriesChange={handleSmallCategoriesChange}
            configJson={configJson}
          />
        )}
        {configEditorMode === "advanced" && (
          <AdvancedConfigFields
            configJson={configJson}
            onChange={handleAdvancedConfigChange}
          />
        )}
        <HealthcheckUrlField initialHealthcheckUrl={initialHealthcheckUrl} />
        <SubmitRow isMutationInFlight={isMutationInFlight} />
      </FieldList>
    </form>
  );
}
