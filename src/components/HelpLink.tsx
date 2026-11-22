/**
 * The "?" next to a number that may raise a question (spec 33 §4): instead of a sentence on the
 * screen it opens Settings → O aplikacji → "Jak to działa" at the right entry.
 */
import { createContext, useContext } from "react";
import { useLanguage } from "../i18n/language";
import { howItWorks, type HowId } from "../lib/howItWorks";

export const HowItWorksContext = createContext<(id: HowId) => void>(() => undefined);

export function HelpLink({ entry }: { entry: HowId }) {
  const { tr } = useLanguage();
  const open = useContext(HowItWorksContext);
  const title = howItWorks(0).find((item) => item.id === entry)?.title ?? "howTitle";
  return (
    <button
      type="button"
      className="help-link"
      aria-label={tr("howOpen", { entry: tr(title) })}
      title={tr("howOpen", { entry: tr(title) })}
      onClick={() => open(entry)}
    >
      ?
    </button>
  );
}
