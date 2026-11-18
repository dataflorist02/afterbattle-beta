/**
 * The first start (makieta RC-Start, author's fixup after STOP 2): one card in the middle of the
 * window, without the navigation, with the logo and three numbered steps — the game folder (done
 * mark once it is set), the address form (the segmented switch of Settings, Polish only) and the
 * update check (yes / no with what it sends). "Zaczynamy" is active only with a folder. Every
 * answer is saved at once, as in Settings. Players coming from a beta see the same card with the
 * update question only (spec 32 §5.1: asked once).
 */
import { useState } from "react";
import type { ReactNode } from "react";
import type { Address } from "../lib/recap/address";
import { useLanguage } from "../i18n/language";
import type { AppSnapshot } from "../types";
import { BrandMark } from "./BrandMark";
import { GameFolderActions } from "./GameFolderActions";
import { ErrorState, SegmentedControl } from "./ui";
import { useUpdates } from "./Updates";

function Step({
  id,
  mark,
  done = false,
  title,
  children,
}: {
  id: string;
  /** The number in the circle; hidden for a single question. */
  mark: number | null;
  done?: boolean;
  title: string;
  children: ReactNode;
}) {
  const { tr } = useLanguage();
  return (
    <div className="welcome-step" role="group" aria-labelledby={`${id}-h`}>
      {mark !== null && (
        <span className={done ? "welcome-dot welcome-dot-done" : "welcome-dot"}>
          {done ? (
            <svg width="16" height="16" viewBox="0 0 24 24" aria-hidden="true">
              <polyline
                points="5 12 10 17 19 7"
                fill="none"
                stroke="currentColor"
                strokeWidth="2.4"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
            </svg>
          ) : (
            <span aria-hidden="true">{mark}</span>
          )}
          <span className="visually-hidden">
            {done ? tr("welcomeStepDone", { step: mark }) : tr("welcomeStep", { step: mark })}
          </span>
        </span>
      )}
      <div className="welcome-step-body">
        <h2 id={`${id}-h`} className="welcome-question">
          {title}
        </h2>
        {children}
      </div>
    </div>
  );
}

/** Yes / no for the update check: nothing chosen until the player answers. */
function UpdateChoice() {
  const { consent, answer } = useUpdates();
  const { tr } = useLanguage();
  return (
    <div className="welcome-choice">
      {(
        [
          [true, "updateAskYes", "on"],
          [false, "updateAskNo", "off"],
        ] as const
      ).map(([enabled, label, value]) => (
        <button
          key={label}
          type="button"
          className="welcome-choice-option"
          aria-pressed={consent === value}
          onClick={() => answer(enabled)}
        >
          {tr(label)}
        </button>
      ))}
    </div>
  );
}

function Shell({
  title,
  lead,
  children,
  action,
}: {
  title: string;
  lead: string;
  children: ReactNode;
  action: ReactNode;
}) {
  const { tr } = useLanguage();
  return (
    <main className="welcome">
      <div className="welcome-inner">
        <header className="welcome-header">
          <BrandMark variant="icon" size={52} />
          <div>
            <h1 className="welcome-title">{title}</h1>
            <p className="welcome-lead">{lead}</p>
          </div>
        </header>
        <section className="card welcome-card" aria-label={tr("welcomeCard")}>
          {children}
        </section>
        <div className="welcome-actions">{action}</div>
      </div>
    </main>
  );
}

export function Welcome({
  snapshot,
  onChanged,
  onDone,
}: {
  snapshot: AppSnapshot;
  onChanged: () => void;
  onDone: () => void;
}) {
  const { language, address, setAddress, tr } = useLanguage();
  const [error, setError] = useState<string | undefined | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
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
  const configured = snapshot.configured;
  const polish = language === "pl";
  const folder = snapshot.sourceStatus.gameRootDisplay;
  return (
    <Shell
      title={tr("welcomeTitle")}
      lead={tr(polish ? "welcomeLead" : "welcomeLeadShort")}
      action={
        <button
          type="button"
          className="button button-primary welcome-start"
          disabled={!configured}
          onClick={onDone}
        >
          {tr("welcomeStart")}
        </button>
      }
    >
      <Step id="welcome-folder" mark={1} done={configured} title={tr("welcomeFolder")}>
        {configured ? (
          <p className="welcome-text">{tr("welcomeFolderFound", { path: folder ?? "" })}</p>
        ) : (
          <>
            <p className="welcome-text">{tr("firstRunText")}</p>
            <p className="welcome-text muted">{tr("firstRunHint")}</p>
          </>
        )}
        {error !== null && <ErrorState code={error} />}
        {notice && (
          <p className="welcome-text muted" role="status">
            {notice}
          </p>
        )}
        <GameFolderActions
          run={run}
          manualLabel={tr(configured ? "welcomeFolderOther" : "firstRunPick")}
        />
      </Step>
      {polish && (
        <Step id="welcome-address" mark={2} title={tr("firstRunAddress")}>
          <p className="welcome-text muted">{tr("welcomeAddressHint")}</p>
          <SegmentedControl<Address>
            label={tr("firstRunAddress")}
            value={address}
            options={[
              { value: "masculine", label: tr("firstRunAddressHe") },
              { value: "feminine", label: tr("firstRunAddressShe") },
              { value: "neutral", label: tr("firstRunAddressNone") },
            ]}
            onChange={setAddress}
          />
        </Step>
      )}
      <Step id="welcome-updates" mark={polish ? 3 : 2} title={tr("updateAskTitle")}>
        <p className="welcome-text muted">{tr("updateAskText")}</p>
        <UpdateChoice />
      </Step>
    </Shell>
  );
}

/** Players coming from a beta: the update question alone, in the same card (spec 32 §5.1). */
export function UpdateWelcome({ onDone }: { onDone: () => void }) {
  const { consent } = useUpdates();
  const { tr } = useLanguage();
  return (
    <Shell
      title={tr("welcomeUpdatesTitle")}
      lead={tr("welcomeUpdatesLead")}
      action={
        <button
          type="button"
          className="button button-primary welcome-start"
          disabled={consent !== "on" && consent !== "off"}
          onClick={onDone}
        >
          {tr("welcomeNext")}
        </button>
      }
    >
      <Step id="welcome-updates" mark={null} title={tr("updateAskTitle")}>
        <p className="welcome-text muted">{tr("updateAskText")}</p>
        <UpdateChoice />
      </Step>
    </Shell>
  );
}
