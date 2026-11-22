/**
 * Settings → O aplikacji → "Jak to działa" (spec 33 §4): short entries that open on a click, the
 * one place that explains how the app counts. A "?" next to a number opens its entry here.
 */
import { useEffect, useState } from "react";
import { useLanguage } from "../i18n/language";
import { howItWorks, type HowId } from "../lib/howItWorks";

export function HowItWorks({
  minutes,
  initial = null,
}: {
  minutes: number;
  initial?: HowId | null;
}) {
  const { tr } = useLanguage();
  const [open, setOpen] = useState<ReadonlySet<HowId>>(() => new Set(initial ? [initial] : []));
  // Opened from a "?" on a screen: the entry is open and in view.
  useEffect(() => {
    if (!initial) return;
    setOpen((current) => new Set([...current, initial]));
    document.getElementById(`how-${initial}`)?.scrollIntoView?.({ block: "center" });
  }, [initial]);
  const toggle = (id: HowId) =>
    setOpen((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  return (
    <section className="how" aria-labelledby="how-h">
      <h3 id="how-h" className="how-title">
        {tr("howTitle")}
      </h3>
      {howItWorks(minutes).map((entry) => {
        const expanded = open.has(entry.id);
        return (
          <div key={entry.id} id={`how-${entry.id}`} className="how-entry">
            <h4 className="how-heading">
              <button
                type="button"
                className="how-toggle"
                aria-expanded={expanded}
                aria-controls={`how-${entry.id}-body`}
                onClick={() => toggle(entry.id)}
              >
                <span>{tr(entry.title)}</span>
                <span className="how-sign" aria-hidden="true">
                  {expanded ? "−" : "+"}
                </span>
              </button>
            </h4>
            <div id={`how-${entry.id}-body`} className="how-body" hidden={!expanded}>
              {entry.paragraphs.map((paragraph) => (
                <p key={paragraph.key}>{tr(paragraph.key, paragraph.params)}</p>
              ))}
            </div>
          </div>
        );
      })}
    </section>
  );
}
