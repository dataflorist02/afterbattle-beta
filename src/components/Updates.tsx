import { createContext, useCallback, useContext, useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { api } from "../api";
import { useLanguage } from "../i18n/language";
import type { AvailableUpdate, UpdateConsent } from "../types";

/** Spec 32 §5.1: the public page of releases, shown in "O aplikacji" (also without consent). */
export const RELEASES_URL = "https://github.com/dataflorist02/afterbattle-beta/releases";

interface Updates {
  /** `null` until the backend answers. */
  consent: UpdateConsent | null;
  answer: (enabled: boolean) => void;
  available: AvailableUpdate | null;
  dismiss: () => void;
  install: () => void;
  installing: boolean;
  installFailed: boolean;
}

const UpdatesContext = createContext<Updates>({
  consent: null,
  answer: () => {},
  available: null,
  dismiss: () => {},
  install: () => {},
  installing: false,
  installFailed: false,
});

export const useUpdates = () => useContext(UpdatesContext);

/**
 * Spec 32 §5: the consent to the update check and the one check per start. Nothing is asked of
 * the network before an explicit "yes" (the backend has the same gate); a found update is only
 * offered, it installs on the player's click.
 */
export function UpdatesProvider({ children }: { children: ReactNode }) {
  const [consent, setConsent] = useState<UpdateConsent | null>(null);
  const [available, setAvailable] = useState<AvailableUpdate | null>(null);
  const [installing, setInstalling] = useState(false);
  const [installFailed, setInstallFailed] = useState(false);
  const checked = useRef(false);

  const check = useCallback(() => {
    if (checked.current) return;
    checked.current = true;
    // A failed check (offline, GitHub down) stays silent: the next start tries again.
    api.checkForUpdate().then(setAvailable, () => setAvailable(null));
  }, []);

  useEffect(() => {
    let live = true;
    api.updateConsent().then(
      (value) => {
        if (!live) return;
        setConsent(value);
        if (value === "on") check();
      },
      // Unknown consent: no question and no request.
      () => live && setConsent("off"),
    );
    return () => {
      live = false;
    };
  }, [check]);

  const answer = useCallback(
    (enabled: boolean) => {
      api.setUpdateConsent(enabled).then(
        (value) => {
          setConsent(value);
          if (value === "on") check();
          else setAvailable(null);
        },
        // Not saved: the question or the switch stays as it was, and nothing is asked.
        () => {},
      );
    },
    [check],
  );
  const install = useCallback(() => {
    setInstalling(true);
    setInstallFailed(false);
    // On success the app restarts; anything coming back is a failure.
    api.installUpdate().then(
      () => setInstalling(false),
      () => {
        setInstalling(false);
        setInstallFailed(true);
      },
    );
  }, []);

  return (
    <UpdatesContext.Provider
      value={{
        consent,
        answer,
        available,
        dismiss: () => setAvailable(null),
        install,
        installing,
        installFailed,
      }}
    >
      {children}
    </UpdatesContext.Provider>
  );
}

/** "Dostępna wersja X" (makieta RC-Aktualizacja): notes, "Później", install on click only. */
export function UpdateNotice() {
  const { available, dismiss, install, installing, installFailed } = useUpdates();
  const { tr } = useLanguage();
  if (!available) return null;
  const title = tr("updateAvailable", { version: available.version });
  return (
    <section className="update-notice" aria-labelledby="update-notice-h">
      <h2 id="update-notice-h" className="update-notice-title">
        {title}
      </h2>
      {available.notes && <p className="update-notice-notes">{available.notes}</p>}
      <p className="update-notice-hint">{tr("updateHint")}</p>
      {installFailed && (
        <p className="update-notice-hint" role="alert">
          {tr("updateFailed")}
        </p>
      )}
      <div className="actions">
        <button type="button" className="button" onClick={dismiss}>
          {tr("updateLater")}
        </button>
        <button
          type="button"
          className="button button-primary"
          disabled={installing}
          onClick={install}
        >
          {tr("updateInstall")}
        </button>
      </div>
    </section>
  );
}
