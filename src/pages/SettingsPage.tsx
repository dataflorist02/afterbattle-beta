import { useState } from "react";
import type { ReactNode } from "react";
import { api } from "../api";
import { GameFolderActions } from "../components/GameFolderActions";
import { Card, Dialog, ErrorState } from "../components/ui";
import { clearGameImages } from "../hooks/useGameImage";
import { useAsync } from "../hooks/useAsync";
import { useDataVersion } from "../app/dataVersion";
import { useLanguage } from "../i18n/language";
import { formatNumber } from "../lib/format";
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

function Section({ id, title, children }: { id: string; title: string; children: ReactNode }) {
  return (
    <Card labelledBy={id} className="section">
      <h2 id={id} className="section-title">
        {title}
      </h2>
      {children}
    </Card>
  );
}

export function SettingsPage({
  snapshot,
  onChanged,
}: {
  snapshot: AppSnapshot;
  onChanged: () => void;
}) {
  const { language, setLanguage, address, setAddress, tr } = useLanguage();
  const [error, setError] = useState<string | undefined | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [minutes, setMinutes] = useState(String(snapshot.inactivityMinutes));
  const [deleting, setDeleting] = useState(false);
  const [typed, setTyped] = useState("");
  const catalog = useAsync(() => api.vehicleClassCatalogStatus(), []);
  const version = useDataVersion();
  const diagnostics = useAsync(() => api.diagnostics(language), [language], {
    refreshKey: version,
  });
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
  const rebuild =
    diagnostics.state.status === "ready" ? rebuildNumbers(diagnostics.state.data) : null;
  const status = snapshot.sourceStatus;
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

      <Section id="game-h" title={tr("settingsGame")}>
        <p className="muted">{status.gameRootDisplay ?? tr("settingsGameNone")}</p>
        <GameFolderActions run={run} manualLabel={tr("settingsPick")}>
          {status.configured &&
            (status.paused ? (
              <button type="button" className="button" onClick={() => run(() => api.resume())}>
                {tr("settingsResume")}
              </button>
            ) : (
              <button type="button" className="button" onClick={() => run(() => api.pause())}>
                {tr("settingsPause")}
              </button>
            ))}
        </GameFolderActions>
      </Section>

      <Section id="import-h" title={tr("settingsImport")}>
        {rebuild && (
          <p className="muted">
            {tr("settingsRebuild", {
              recalculated: formatNumber(rebuild.recalculated, language),
              notRandom: formatNumber(rebuild.notRandom, language),
              missing: formatNumber(rebuild.withoutReplay, language),
              conflicts: formatNumber(rebuild.conflicts, language),
            })}
          </p>
        )}
        {rebuild && (rebuild.skippedPartial > 0 || rebuild.skippedEmptySessions > 0) && (
          <p className="muted">
            {tr("settingsRebuildSkipped", {
              partial: formatNumber(rebuild.skippedPartial, language),
              sessions: formatNumber(rebuild.skippedEmptySessions, language),
            })}
          </p>
        )}
        <div className="actions" style={{ justifyContent: "flex-start" }}>
          <button
            type="button"
            className="button"
            disabled={!status.configured}
            onClick={() => run(() => api.startImport())}
          >
            {tr("settingsImportStart")}
          </button>
        </div>
      </Section>

      <Section id="wn8-h" title={tr("settingsWn8")}>
        <label className="cell">
          <input
            type="checkbox"
            checked={xvmOn}
            onChange={(event) => run(() => api.setXvmEnabled(event.target.checked))}
          />
          {tr("settingsWn8Toggle")}
        </label>
        <p className="muted">{tr("settingsWn8Consent")}</p>
        <p className="muted">
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
        {xvmOn && (
          <div className="actions" style={{ justifyContent: "flex-start" }}>
            <button
              type="button"
              className="button"
              onClick={() => run(() => api.refreshExpectedValues())}
            >
              {tr("settingsWn8Refresh")}
            </button>
          </div>
        )}
      </Section>

      <Section id="catalog-h" title={tr("settingsCatalog")}>
        <p className="muted">
          {catalog.state.status === "ready" && catalog.state.data.vehicleCount
            ? tr("settingsCatalogReady", {
                version: catalog.state.data.sourceVersion ?? "—",
                count: formatNumber(catalog.state.data.vehicleCount, language),
              })
            : tr("settingsCatalogMissing")}
        </p>
      </Section>

      <Section id="prefs-h" title={tr("settingsLanguage")}>
        <div className="filters">
          <label className="field">
            {tr("settingsLanguage")}
            <select
              value={language}
              onChange={(event) => setLanguage(event.target.value as Language)}
            >
              <option value="pl">Polski</option>
              <option value="en">English</option>
            </select>
          </label>
          <label className="field">
            {tr("settingsAddress")}
            <select value={address} onChange={(event) => setAddress(event.target.value as Address)}>
              <option value="neutral">{tr("settingsAddressNeutral")}</option>
              <option value="masculine">{tr("settingsAddressMasculine")}</option>
              <option value="feminine">{tr("settingsAddressFeminine")}</option>
            </select>
          </label>
          <label className="field">
            {tr("settingsInactivity")}
            <input
              type="number"
              min={5}
              max={720}
              value={minutes}
              onChange={(event) => setMinutes(event.target.value)}
            />
          </label>
          <button
            type="button"
            className="button"
            onClick={() =>
              run(() => api.updatePreferences(language, Number(minutes)), tr("settingsSaved"))
            }
          >
            {tr("settingsSave")}
          </button>
        </div>
      </Section>

      <Section id="privacy-h" title={tr("settingsPrivacy")}>
        <p className="muted">{tr("privacyLocal")}</p>
        <p className="muted">{tr("privacyImages")}</p>
        <p className="muted">{tr("privacyNames")}</p>
        <p className="muted">{tr("privacyRoster")}</p>
        <div className="actions" style={{ justifyContent: "flex-start" }}>
          <button type="button" className="button" onClick={() => setDeleting(true)}>
            {tr("deleteAll")}
          </button>
        </div>
      </Section>

      <Section id="diag-h" title={tr("settingsDiagnostics")}>
        <div className="actions" style={{ justifyContent: "flex-start" }}>
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
        </div>
      </Section>

      <Section id="about-h" title={tr("settingsAbout")}>
        <p className="muted">{tr("aboutText", { version: APP_VERSION })}</p>
        <p className="muted">
          {tr("footerWn8")}:{" "}
          {expected.sourceVersion ? `XVM · ${expected.sourceVersion}` : tr("footerWn8Off")}
        </p>
      </Section>

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
