import { useState, type ChangeEvent } from "react";
import { FieldList, FormField } from "../../components/ui/FormField";
import { OptionList, OptionRow } from "../../components/ui/OptionList";
import TextInput from "../../components/ui/TextInput";
import Select from "../../components/ui/Select";
import { Button } from "../../components/ui/Button";

interface SessionFormProps {
  initialName: string;
  initialConfig: string;
  initialHealthcheckUrl: string;
  isMutationInFlight: boolean;
  onSubmit: (formData: FormData) => void | Promise<void>;
  /**
   * When set, renders a Location <select> as the first field (name
   * "locationId") for the caller to read from the submitted FormData. Only the
   * QR-code kiosk enrollment form passes this — every other caller already has
   * a location from context and keeps it out of the form entirely.
   */
  locations?: ReadonlyArray<{ readonly id: string; readonly name: string }>;
  /**
   * Set to false to omit the Health Check URL field entirely. The QR-code kiosk
   * enrollment form hides it: the kiosk isn't provisioned yet at that point, so
   * there's nothing to health-check, and the field can be added later from the
   * session's edit page.
   */
  showHealthcheckUrl?: boolean;
}

type ConfigEditorMode = "basic" | "advanced";
type SessionMode = "scan" | "status";
type KioskTheme = "auto" | "light" | "dark";
type ConfigObject = Record<string, unknown>;

interface SegmentedControlProps<T extends string> {
  label: string;
  value: T;
  options: { value: T; label: string }[];
  onChange: (nextValue: T) => void;
}

interface ConfigEditorModeControlProps {
  configEditorMode: ConfigEditorMode;
  onSetEditorMode: (nextEditorMode: ConfigEditorMode) => void;
}

interface ThemeControlProps {
  theme: KioskTheme;
  onChange: (nextTheme: KioskTheme) => void;
}

interface BasicSessionModeFieldsProps {
  sessionMode: SessionMode;
  onChange: (nextMode: SessionMode) => void;
  theme: KioskTheme;
  onThemeChange: (next: KioskTheme) => void;
  smallCategories: boolean;
  onSmallCategoriesChange: (next: boolean) => void;
  easyTimeEntry: boolean;
  onEasyTimeEntryChange: (next: boolean) => void;
  guests: boolean;
  onGuestsChange: (next: boolean) => void;
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

/**
 * Omits the key entirely for Auto rather than writing `theme: "auto"`, since an
 * omitted key already means auto to the kiosk (see `themeFromConfig` in
 * KioskMain) — matched by `getThemeFromConfig` below reading it back the same way.
 */
function withTheme(config: ConfigObject, theme: KioskTheme): ConfigObject {
  const next = { ...config };
  if (theme === "auto") {
    delete next.theme;
  } else {
    next.theme = theme;
  }
  return next;
}

/**
 * Mirrors the kiosk's own reading of the key (see `themeFromConfig` in
 * KioskMain): an omitted key is auto, `"dark"` is dark, and anything else is light.
 */
function getThemeFromConfig(config: ConfigObject): KioskTheme {
  if (config.theme === undefined || config.theme === "auto") {
    return "auto";
  }
  return config.theme === "dark" ? "dark" : "light";
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

function withGuests(config: ConfigObject, enabled: boolean): ConfigObject {
  const next = { ...config };
  if (enabled) {
    next.guests = true;
  } else {
    delete next.guests;
  }
  return next;
}

function getGuestsFromConfig(config: ConfigObject): boolean {
  return !!config.guests;
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

function LocationField({
  locations,
}: {
  locations: ReadonlyArray<{ readonly id: string; readonly name: string }>;
}) {
  return (
    <FormField label={<label htmlFor="locationId">Location</label>}>
      <Select name="locationId" id="locationId" required defaultValue="">
        <option value="" disabled>
          Select a location…
        </option>
        {locations.map((location) => (
          <option key={location.id} value={location.id}>
            {location.name}
          </option>
        ))}
      </Select>
    </FormField>
  );
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

function SegmentedControl<T extends string>({
  label,
  value,
  options,
  onChange,
}: SegmentedControlProps<T>) {
  return (
    <div
      className="inline-flex overflow-hidden rounded-lg border border-line-strong"
      role="group"
      aria-label={label}
    >
      {options.map((option, index) => (
        <button
          key={option.value}
          className={`m-0 min-w-23 cursor-pointer rounded-none border-0 bg-surface-raised px-3 py-1.5 text-ink hover:bg-surface-sunken aria-pressed:bg-navy aria-pressed:text-white aria-pressed:hover:bg-[#2b4f97] ${index > 0 ? "border-l border-line-strong" : ""}`}
          type="button"
          onClick={() => onChange(option.value)}
          aria-pressed={value === option.value}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}

function ConfigEditorModeControl({
  configEditorMode,
  onSetEditorMode,
}: ConfigEditorModeControlProps) {
  return (
    <FormField label={<span>Config Editor</span>}>
      <SegmentedControl
        label="Config editor mode"
        value={configEditorMode}
        options={[
          { value: "basic", label: "Basic" },
          { value: "advanced", label: "Advanced" },
        ]}
        onChange={onSetEditorMode}
      />
    </FormField>
  );
}

function ThemeControl({ theme, onChange }: ThemeControlProps) {
  return (
    <FormField label={<span>Theme</span>}>
      <SegmentedControl
        label="Theme"
        value={theme}
        options={[
          { value: "auto", label: "Auto" },
          { value: "light", label: "Light" },
          { value: "dark", label: "Dark" },
        ]}
        onChange={onChange}
      />
      <p className="mt-1.5 mb-0 text-ink-muted">
        Auto follows the device's own light/dark setting, which may require
        configuration of the browser and/or the operating system to work. Light
        and Dark pin the kiosk to that theme regardless of the device.
      </p>
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
  guests,
  onGuestsChange,
  quickPickCategories,
  onQuickPickCategoriesChange,
  configJson,
  theme,
  onThemeChange,
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
      <ThemeControl theme={theme} onChange={onThemeChange} />
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
                  checked={guests}
                  onChange={(e) => onGuestsChange(e.target.checked)}
                  className="mt-0.5"
                />
              }
              title={
                <span className="inline-flex items-center gap-2">
                  Guests
                  <span className="rounded-full bg-blue-600 px-1.5 py-0.5 text-[0.625rem] font-semibold tracking-wide text-white uppercase dark:bg-blue-500">
                    Beta
                  </span>
                </span>
              }
              description="show a Guest button so non-members can be signed in and out by name without a membership record"
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
  locations,
  showHealthcheckUrl = true,
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
  const guests = getGuestsFromConfig(parsedConfig);
  const quickPickCategories = getQuickPickCategoriesFromConfig(parsedConfig);
  const theme = getThemeFromConfig(parsedConfig);

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

  function handleGuestsChange(enabled: boolean) {
    const nextConfig = withGuests(parseConfigObject(configJson), enabled);
    setConfigJson(JSON.stringify(nextConfig, null, 2));
  }

  function handleQuickPickCategoriesChange(enabled: boolean) {
    const nextConfig = withQuickPickCategories(
      parseConfigObject(configJson),
      enabled,
    );
    setConfigJson(JSON.stringify(nextConfig, null, 2));
  }

  function handleThemeChange(nextTheme: KioskTheme) {
    const nextConfig = withTheme(parseConfigObject(configJson), nextTheme);
    setConfigJson(JSON.stringify(nextConfig, null, 2));
  }

  function handleAdvancedConfigChange(event: ChangeEvent<HTMLTextAreaElement>) {
    const nextConfigText = event.target.value;
    setConfigJson(nextConfigText);
  }

  return (
    <form action={onSubmit}>
      <FieldList>
        {locations != null && <LocationField locations={locations} />}
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
            guests={guests}
            onGuestsChange={handleGuestsChange}
            quickPickCategories={quickPickCategories}
            onQuickPickCategoriesChange={handleQuickPickCategoriesChange}
            configJson={configJson}
            theme={theme}
            onThemeChange={handleThemeChange}
          />
        )}
        {configEditorMode === "advanced" && (
          <AdvancedConfigFields
            configJson={configJson}
            onChange={handleAdvancedConfigChange}
          />
        )}
        {showHealthcheckUrl && (
          <HealthcheckUrlField initialHealthcheckUrl={initialHealthcheckUrl} />
        )}
        <SubmitRow isMutationInFlight={isMutationInFlight} />
      </FieldList>
    </form>
  );
}
