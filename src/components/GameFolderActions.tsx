import { useState } from "react";
import type { ReactNode } from "react";
import { api } from "../api";
import { clearGameImages } from "../hooks/useGameImage";
import { useLanguage } from "../i18n/language";

/** Runs an action and reports its failure code (or the `done` notice) to the page. */
export type RunAction = (action: () => Promise<unknown>, done?: string) => void;

/**
 * Choosing the game folder (hotfix 1.0.0-beta.3): detection with its results, and the manual
 * choice through the system folder picker. The chosen path goes to the backend unchanged; it
 * normalizes and checks it and answers with a specific error.
 */
export function GameFolderActions({
  run,
  manualLabel,
  children,
}: {
  run: RunAction;
  manualLabel: string;
  children?: ReactNode;
}) {
  const { language, tr } = useLanguage();
  const [folders, setFolders] = useState<string[] | null>(null);
  const use = (folder: string) =>
    run(() => {
      clearGameImages();
      return api.configureLocal(folder, language);
    }, tr("settingsSaved"));
  const pick = () =>
    void api.pickGameFolder(tr("gameFolderPickTitle")).then(
      (chosen) => {
        if (chosen !== null) use(chosen);
      },
      (failure: unknown) => run(() => Promise.reject(failure)),
    );
  return (
    <>
      <div className="actions" style={{ justifyContent: "flex-start" }}>
        <button
          type="button"
          className="button"
          onClick={() => run(() => api.detectInstallations().then(setFolders))}
        >
          {tr("settingsDetect")}
        </button>
        <button type="button" className="button" onClick={pick}>
          {manualLabel}
        </button>
        {children}
      </div>
      {folders?.length === 0 && <p className="muted">{tr("gameFolderNotFound")}</p>}
      {folders?.map((folder) => (
        <div key={folder} className="stat-row">
          <span>{folder}</span>
          <button type="button" className="button button-primary" onClick={() => use(folder)}>
            {tr("settingsUse")}
          </button>
        </div>
      ))}
    </>
  );
}
