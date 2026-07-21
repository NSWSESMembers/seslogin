import { useState, type ChangeEvent } from "react";
import { FieldList, FormField } from "../../components/ui/FormField";
import { OptionList, OptionRow } from "../../components/ui/OptionList";
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
  easyTimeEntry: boolean;
  onEasyTimeEntryChange: (next: boolean) => void;
  newCategories: boolean;
  onNewCategoriesChange: (next: boolean) => void;
  quickPickCategories: boolean;
  onQuickPickCategoriesChange: (next: boolean) => void;
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

function withEasyTimeEntry(
  config: ConfigObject,
  enabled: boolean,
): ConfigObject {
  const next = { ...config };
  if (enabled) {
    next.easyTimeEntry = true;
  } else {
    delete next.easyTimeEntry;
  }
  return next;
}

function getEasyTimeEntryFromConfig(config: ConfigObject): boolean {
  return !!config.easyTimeEntry;
}

function withNewCategories(
  config: ConfigObject,
  enabled: boolean,
): ConfigObject {
  const next = { ...config };
  if (enabled) {
    next.newCategories = true;
  } else {
    delete next.newCategories;
  }
  return next;
}

function getNewCategoriesFromConfig(config: ConfigObject): boolean {
  return !!config.newCategories;
}

function withQuickPickCategories(
  config: ConfigObject,
  enabled: boolean,
): ConfigObject {
  const next = { ...config };
  if (enabled) {
    next.quickPickCategories = true;
  } else {
    delete next.quickPickCategories;
  }
  return next;
}

function getQuickPickCategoriesFromConfig(config: ConfigObject): boolean {
  return !!config.quickPickCategories;
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
        className="inline-flex overflow-hidden rounded-lg border border-line-strong"
        role="group"
        aria-label="Config editor mode"
      >
        <button
          className="m-0 min-w-23 cursor-pointer rounded-none border-0 bg-surface-raised px-3 py-1.5 text-ink hover:bg-surface-sunken aria-pressed:bg-navy aria-pressed:text-white aria-pressed:hover:bg-[#2b4f97]"
          type="button"
          onClick={() => onSetEditorMode("basic")}
          aria-pressed={configEditorMode === "basic"}
        >
          Basic
        </button>
        <button
          className="m-0 min-w-23 cursor-pointer rounded-none border-0 border-l border-line-strong bg-surface-raised px-3 py-1.5 text-ink hover:bg-surface-sunken aria-pressed:bg-navy aria-pressed:text-white aria-pressed:hover:bg-[#2b4f97]"
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
  easyTimeEntry,
  onEasyTimeEntryChange,
  newCategories,
  onNewCategoriesChange,
  quickPickCategories,
  onQuickPickCategoriesChange,
  configJson,
}: BasicSessionModeFieldsProps) {
  return (
    <>
      <FormField label={<span>Mode</span>}>
        <OptionList role="radiogroup" aria-label="Mode">
          <OptionRow
            input={
              <input
                type="radio"
                name="sessionMode"
                value="scan"
                checked={sessionMode === "scan"}
                onChange={() => onChange("scan")}
                className="mt-0.5"
              />
            }
            title="Scan"
            description="allow members to sign in and out on this computer (touchscreen or mouse and keyboard required)"
          />
          <OptionRow
            input={
              <input
                type="radio"
                name="sessionMode"
                value="status"
                checked={sessionMode === "status"}
                onChange={() => onChange("status")}
                className="mt-0.5"
              />
            }
            title="Status"
            description="show a live-updating non-interactive list of who is currently signed in at the unit along with how long they've been signed in for"
          />
        </OptionList>
        <input type="hidden" name="config" value={configJson} />
      </FormField>
      {sessionMode === "scan" && (
        <FormField label={<span>Options</span>}>
          <OptionList>
            <OptionRow
              input={
                <input
                  type="checkbox"
                  checked={smallCategories}
                  onChange={(e) => onSmallCategoriesChange(e.target.checked)}
                  className="mt-0.5"
                />
              }
              title="Small categories"
              description="use smaller category buttons to fit more on screen — useful on smaller or lower-resolution displays"
            />
            <OptionRow
              input={
                <input
                  type="checkbox"
                  checked={easyTimeEntry}
                  onChange={(e) => onEasyTimeEntryChange(e.target.checked)}
                  className="mt-0.5"
                />
              }
              title="Easy time entry"
              description="use a touch-friendly 12-hour keypad with an explicit confirm step and quick Yesterday/Today buttons on the sign-out Adjust screen, instead of the default 24-hour numeric keypad"
            />
            <OptionRow
              input={
                <input
                  type="checkbox"
                  checked={newCategories}
                  onChange={(e) => onNewCategoriesChange(e.target.checked)}
                  className="mt-0.5"
                />
              }
              title="New categories"
              description="use the reworked category list on the sign-out screens — new icon artwork, with several retired subcategories removed and others reordered"
            />
            <OptionRow
              input={
                <input
                  type="checkbox"
                  checked={quickPickCategories}
                  onChange={(e) =>
                    onQuickPickCategoriesChange(e.target.checked)
                  }
                  className="mt-0.5"
                />
              }
              title="Quick pick categories"
              description="on the sign-out screen, show quick-pick buttons for the location's and the member's own recently-used categories before the full category list, so people converge on the same categories instead of picking slightly different ones each time"
            />
          </OptionList>
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
      <p className="mt-1.5 mb-0 text-ink-muted">
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
        className="w-full rounded-md border border-line p-2 font-mono text-sm"
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
  const easyTimeEntry = getEasyTimeEntryFromConfig(parsedConfig);
  const newCategories = getNewCategoriesFromConfig(parsedConfig);
  const quickPickCategories = getQuickPickCategoriesFromConfig(parsedConfig);

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

  function handleEasyTimeEntryChange(enabled: boolean) {
    const nextConfig = withEasyTimeEntry(
      parseConfigObject(configJson),
      enabled,
    );
    setConfigJson(JSON.stringify(nextConfig, null, 2));
  }

  function handleNewCategoriesChange(enabled: boolean) {
    const nextConfig = withNewCategories(
      parseConfigObject(configJson),
      enabled,
    );
    setConfigJson(JSON.stringify(nextConfig, null, 2));
  }

  function handleQuickPickCategoriesChange(enabled: boolean) {
    const nextConfig = withQuickPickCategories(
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
            easyTimeEntry={easyTimeEntry}
            onEasyTimeEntryChange={handleEasyTimeEntryChange}
            newCategories={newCategories}
            onNewCategoriesChange={handleNewCategoriesChange}
            quickPickCategories={quickPickCategories}
            onQuickPickCategoriesChange={handleQuickPickCategoriesChange}
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
