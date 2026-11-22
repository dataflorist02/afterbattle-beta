import { useEffect, useState } from "react";
import type { ReactNode } from "react";
import { api } from "../api";
import { GameFolderActions } from "../components/GameFolderActions";
import { Card, Dialog, ErrorState, SegmentedControl } from "../components/ui";
import { RELEASES_URL, useUpdates } from "../components/Updates";
import { clearGameImages } from "../hooks/useGameImage";
import { useAsync } from "../hooks/useAsync";
import { useDataVersion } from "../app/dataVersion";
import { useLanguage } from "../i18n/language";
import { formatNumber } from "../lib/format";
import { SAVED_SHOW_MS, prefersReducedMotion } from "../lib/motion";
import { EmberFlash } from "../components/Ember";
import { HowItWorks } from "../components/HowItWorks";
import type { HowId } from "../lib/howItWorks";
import { APP_VERSION } from "../release";
import type { Address } from "../lib/recap/address";
import type { AppSnapshot, Language } from "../types";

interface RebuildNumbers {
  recalculated: number;
  notRandom: number;
  withoutReplay: number;
  conflicts: number;
  skippedPartial: number;
  skippedEmptySessions: number;
}

/** The rebuild report lives in the diagnostic report (E2); shown here in words. */
export function rebuildNumbers(diagnostics: string): RebuildNumbers | null {
  try {
    const report = (JSON.parse(diagnostics) as { rebuildReport?: Record<string, unknown> | null })
      .rebuildReport;
    if (!report) return null;
    const pick = (camel: string, snake: string) =>
      Number((report[camel] ?? report[snake]) as number | undefined) || 0;
    return {
      recalculated: pick("recalculated", "recalculated"),
      notRandom: pick("notRandom", "not_random"),
      withoutReplay: pick("withoutReplay", "without_replay"),
      conflicts: pick("conflicts", "conflicts"),
      skippedPartial: pick("skippedPartial", "skipped_partial"),
      skippedEmptySessions: pick("skippedEmptySessions", "skipped_empty_sessions"),
    };
  } catch {
    return null;
  }
}

/** Inactivity limits the backend accepts (minutes). */
const MINUTES_MIN = 5;
const MINUTES_MAX = 720;

const GROUP_IDS = [
  "settings-game",
  "settings-you",
  "settings-data",
  "settings-privacy",
  "settings-about",
] as const;

/** Spec 31 §4: a group of options, a target of the list of links. */
function Group({ id, title, children }: { id: string; title: string; children: ReactNode }) {
  return (
    <Card id={id} labelledBy={`${id}-h`} className="section settings-group">
      <h2 id={`${id}-h`} className="section-title">
        {title}
      </h2>
      {children}
    </Card>
  );
}

/** One option: label, one sentence about it, its state, and its control. */
function Option({
  label,
  description,
  state,
  controlId,
  children,
}: {
  label: string;
  description: string;
  state?: ReactNode;
  controlId?: string;
  children?: ReactNode;
}) {
  return (
    <div className="setting">
      <div className="setting-text">
        {controlId ? (
          <label className="setting-label" htmlFor={controlId}>
            {label}
          </label>
        ) : (
          <span className="setting-label">{label}</span>
        )}
        <p className="setting-desc">{description}</p>
        {state}
      </div>
      {children && <div className="setting-control">{children}</div>}
    </div>
  );
}

export function SettingsPage({
  snapshot,
  onChanged,
  openHow = null,
}: {
  snapshot: AppSnapshot;
  onChanged: () => void;
  /** Opened from a "?" on a screen: this "Jak to działa" entry is open (spec 33 §4). */
  openHow?: HowId | null;
}) {
  const { language, setLanguage, address, setAddress, tr } = useLanguage();
  const updates = useUpdates();
  const [error, setError] = useState<string | undefined | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [minutes, setMinutes] = useState(String(snapshot.inactivityMinutes));
  const [savedMinutes, setSavedMinutes] = useState(snapshot.inactivityMinutes);
  const [saved, setSaved] = useState(0);
  const [savedShown, setSavedShown] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [typed, setTyped] = useState("");
  const catalog = useAsync(() => api.vehicleClassCatalogStatus(), []);
  const version = useDataVersion();
  const diagnostics = useAsync(() => api.diagnostics(language), [language], {
    refreshKey: version,
  });
  // "Zapisano." shows after each save and fades out (spec 31 §2 motion list).
  useEffect(() => {
    if (!saved) return;
    const timer = window.setTimeout(() => setSavedShown(false), SAVED_SHOW_MS);
    return () => window.clearTimeout(timer);
  }, [saved]);
  const expected = snapshot.wn8Presentation.expectedValues;
  const xvmOn = snapshot.advancedStatus.xvmEnabled;
  const run = (action: () => Promise<unknown>, done?: string) => {
    setError(null);
    setNotice(null);
    action().then(
      () => {
        if (done) setNotice(done);
        onChanged();
      },
      (failure: { code?: string }) => setError(failure?.code),
    );
  };
  // Spec 31 §4: language, address and the inactivity limit save at once, without "Zapisz".
  const markSaved = () => {
    setSaved((count) => count + 1);
    setSavedShown(true);
  };
  const savePreferences = (nextLanguage: Language, nextMinutes: number) => {
    setError(null);
    api.updatePreferences(nextLanguage, nextMinutes).then(
      () => {
        setSavedMinutes(nextMinutes);
        markSaved();
        onChanged();
      },
      (failure: { code?: string }) => setError(failure?.code),
    );
  };
  const chooseLanguage = (next: Language) => {
    setLanguage(next);
    savePreferences(next, savedMinutes);
  };
  const chooseAddress = (next: Address) => {
    setAddress(next);
    markSaved();
  };
  const commitMinutes = () => {
    const value = Number(minutes);
    if (!Number.isInteger(value) || value < MINUTES_MIN || value > MINUTES_MAX) {
      setMinutes(String(savedMinutes));
      return;
    }
    if (value !== savedMinutes) savePreferences(language, value);
  };
  const rebuild =
    diagnostics.state.status === "ready" ? rebuildNumbers(diagnostics.state.data) : null;
  const status = snapshot.sourceStatus;
  const groups = [
    [GROUP_IDS[0], tr("settingsGroupGame")],
    [GROUP_IDS[1], tr("settingsGroupYou")],
    [GROUP_IDS[2], tr("settingsGroupData")],
    [GROUP_IDS[3], tr("settingsGroupPrivacy")],
    [GROUP_IDS[4], tr("settingsAbout")],
  ] as const;
  const title = Object.fromEntries(groups) as Record<(typeof groups)[number][0], string>;
  const [currentGroup, setCurrentGroup] = useState<string>(groups[0][0]);
  // The group in view is marked in the list of links (author's fixup after STOP 1): the first
  // group crossing the band near the top of the window, in page order.
  useEffect(() => {
    if (typeof IntersectionObserver === "undefined") return;
    const ids: readonly string[] = GROUP_IDS;
    const visible = new Set<string>();
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries)
          if (entry.isIntersecting) visible.add(entry.target.id);
          else visible.delete(entry.target.id);
        const first = ids.find((id) => visible.has(id));
        if (first) setCurrentGroup(first);
      },
      { rootMargin: "-20% 0px -60% 0px" },
    );
    for (const id of ids) {
      const element = document.getElementById(id);
      if (element) observer.observe(element);
    }
    return () => observer.disconnect();
  }, []);
  return (
    <>
      <header className="page-header">
        <h1 className="page-title">{tr("settingsTitle")}</h1>
      </header>
      {error !== null && <ErrorState code={error} />}
      {notice && (
        <p className="muted" role="status">
          {notice}
        </p>
      )}

      <div className="settings-layout">
        <nav className="settings-nav" aria-label={tr("settingsNav")}>
          <ul>
            {groups.map(([id, name]) => (
              <li key={id}>
                <a href={`#${id}`} aria-current={currentGroup === id ? "true" : undefined}>
                  {name}
                </a>
              </li>
            ))}
          </ul>
        </nav>

        <div className="settings-groups">
          <Group id="settings-game" title={title["settings-game"]}>
            <Option
              label={tr("settingsGame")}
              description={tr("settingsGameDesc")}
              state={
                <p className="setting-state">{status.gameRootDisplay ?? tr("settingsGameNone")}</p>
              }
            >
              <GameFolderActions run={run} manualLabel={tr("settingsPick")} />
            </Option>
            {status.configured && (
              <Option label={tr("settingsSaving")} description={tr("settingsSavingDesc")}>
                <input
                  type="checkbox"
                  className="switch"
                  aria-label={tr("settingsSaving")}
                  checked={!status.paused}
                  onChange={(event) =>
                    run(() => (event.target.checked ? api.resume() : api.pause()))
                  }
                />
              </Option>
            )}
          </Group>

          <Group id="settings-you" title={title["settings-you"]}>
            <Option label={tr("settingsLanguage")} description={tr("settingsLanguageDesc")}>
              <SegmentedControl
                label={tr("settingsLanguage")}
                value={language}
                options={[
                  { value: "pl", label: "Polski" },
                  { value: "en", label: "English" },
                ]}
                onChange={chooseLanguage}
              />
            </Option>
            <Option label={tr("settingsAddress")} description={tr("settingsAddressDesc")}>
              <SegmentedControl<Address>
                label={tr("settingsAddress")}
                value={address}
                options={[
                  { value: "masculine", label: tr("firstRunAddressHe") },
                  { value: "feminine", label: tr("firstRunAddressShe") },
                  { value: "neutral", label: tr("firstRunAddressNone") },
                ]}
                onChange={chooseAddress}
              />
            </Option>
            <Option
              label={tr("settingsInactivity")}
              description={tr("settingsInactivityDesc")}
              controlId="settings-inactivity"
            >
              <input
                id="settings-inactivity"
                type="number"
                min={MINUTES_MIN}
                max={MINUTES_MAX}
                value={minutes}
                onChange={(event) => setMinutes(event.target.value)}
                onBlur={commitMinutes}
                onKeyDown={(event) => {
                  if (event.key === "Enter") commitMinutes();
                }}
              />
            </Option>
            {saved > 0 && (
              <p className="setting-saved" role="status" data-shown={savedShown}>
                {tr("settingsSaved")}
                {/* Spec 33 §3: Settings stay cool; a save flashes once. */}
                {!prefersReducedMotion() && <EmberFlash key={saved} />}
              </p>
            )}
          </Group>

          <Group id="settings-data" title={title["settings-data"]}>
            <Option
              label={tr("settingsImport")}
              description={tr("settingsImportDesc")}
              state={
                <>
                  {rebuild && (
                    <p className="setting-state">
                      {tr("settingsRebuild", {
                        recalculated: formatNumber(rebuild.recalculated, language),
                        notRandom: formatNumber(rebuild.notRandom, language),
                        missing: formatNumber(rebuild.withoutReplay, language),
                        conflicts: formatNumber(rebuild.conflicts, language),
                      })}
                    </p>
                  )}
                  {rebuild && (rebuild.skippedPartial > 0 || rebuild.skippedEmptySessions > 0) && (
                    <p className="setting-state">
                      {tr("settingsRebuildSkipped", {
                        partial: formatNumber(rebuild.skippedPartial, language),
                        sessions: formatNumber(rebuild.skippedEmptySessions, language),
                      })}
                    </p>
                  )}
                </>
              }
            >
              <button
                type="button"
                className="button"
                disabled={!status.configured}
                onClick={() => run(() => api.startImport())}
              >
                {tr("settingsImportStart")}
              </button>
            </Option>
            <Option
              label={tr("settingsCatalog")}
              description={tr("settingsCatalogDesc")}
              state={
                <p className="setting-state">
                  {catalog.state.status === "ready" && catalog.state.data.vehicleCount
                    ? tr("settingsCatalogReady", {
                        version: catalog.state.data.sourceVersion ?? "—",
                        count: formatNumber(catalog.state.data.vehicleCount, language),
                      })
                    : tr("settingsCatalogMissing")}
                </p>
              }
            />
            <Option
              label={tr("settingsWn8")}
              description={tr("settingsWn8Desc")}
              state={
                <p className="setting-state">
                  {expected.sourceVersion
                    ? tr("settingsWn8Version", {
                        version: expected.sourceVersion,
                        count: formatNumber(expected.vehicleCount, language),
                      })
                    : tr("settingsWn8None")}
                  {expected.skippedRowCount
                    ? ` ${tr("settingsWn8Skipped", { count: expected.skippedRowCount })}`
                    : ""}
                </p>
              }
            >
              <input
                type="checkbox"
                className="switch"
                aria-label={tr("settingsWn8Toggle")}
                checked={xvmOn}
                onChange={(event) => run(() => api.setXvmEnabled(event.target.checked))}
              />
              {xvmOn && (
                <button
                  type="button"
                  className="button"
                  onClick={() => run(() => api.refreshExpectedValues())}
                >
                  {tr("settingsWn8Refresh")}
                </button>
              )}
            </Option>
          </Group>

          <Group id="settings-privacy" title={title["settings-privacy"]}>
            <Option label={tr("settingsUpdates")} description={tr("settingsUpdatesDesc")}>
              <input
                type="checkbox"
                className="switch"
                aria-label={tr("settingsUpdates")}
                checked={updates.consent === "on"}
                disabled={updates.consent === null}
                onChange={(event) => updates.answer(event.target.checked)}
              />
            </Option>
            <Option
              label={tr("settingsPrivacyStays")}
              description={tr("settingsPrivacyStaysDesc")}
              state={
                <ul className="setting-state setting-list">
                  <li>{tr("privacyLocal")}</li>
                  <li>{tr("privacyImages")}</li>
                  <li>{tr("privacyNames")}</li>
                  <li>{tr("privacyRoster")}</li>
                </ul>
              }
            />
            <Option label={tr("deleteAll")} description={tr("settingsDeleteDesc")}>
              <button
                type="button"
                className="button button-danger"
                onClick={() => setDeleting(true)}
              >
                {tr("deleteAll")}
              </button>
            </Option>
          </Group>

          <Group id="settings-about" title={title["settings-about"]}>
            <Option
              label={tr("settingsVersion")}
              description={tr("settingsVersionDesc")}
              state={<p className="setting-state">Afterbattle {APP_VERSION}</p>}
            />
            <Option
              label={tr("settingsReleases")}
              description={tr("settingsReleasesDesc")}
              state={<p className="setting-state setting-url">{RELEASES_URL}</p>}
            >
              <button
                type="button"
                className="button"
                onClick={() =>
                  run(
                    () => navigator.clipboard.writeText(RELEASES_URL),
                    tr("settingsReleasesCopied"),
                  )
                }
              >
                {tr("settingsReleasesCopy")}
              </button>
            </Option>
            <Option label={tr("settingsDiagnostics")} description={tr("settingsDiagnosticsDesc")}>
              <button
                type="button"
                className="button"
                disabled={diagnostics.state.status !== "ready"}
                onClick={() => {
                  if (diagnostics.state.status === "ready")
                    run(
                      () =>
                        navigator.clipboard.writeText(
                          diagnostics.state.status === "ready" ? diagnostics.state.data : "",
                        ),
                      tr("diagnosticsCopied"),
                    );
                }}
              >
                {tr("diagnosticsCopy")}
              </button>
            </Option>
            <HowItWorks minutes={snapshot.inactivityMinutes} initial={openHow} />
          </Group>
        </div>
      </div>

      {deleting && (
        <Dialog
          title={tr("deleteAll")}
          onClose={() => {
            setDeleting(false);
            setTyped("");
          }}
        >
          <p className="muted">{tr("deleteAllText", { word: tr("deleteWord") })}</p>
          <input
            type="text"
            aria-label={tr("deleteWord")}
            value={typed}
            onChange={(event) => setTyped(event.target.value)}
          />
          <div className="actions">
            <button
              type="button"
              className="button"
              onClick={() => {
                setDeleting(false);
                setTyped("");
              }}
            >
              {tr("cancel")}
            </button>
            <button
              type="button"
              className="button button-primary"
              disabled={typed.trim() !== tr("deleteWord")}
              onClick={() => {
                setDeleting(false);
                setTyped("");
                run(() => {
                  clearGameImages();
                  return api.deleteAllData();
                }, tr("deleteDone"));
              }}
            >
              {tr("deleteAll")}
            </button>
          </div>
        </Dialog>
      )}
    </>
  );
}
