/**
 * A template summary card (spec 30 §8): the sentences as a list, each followed by references
 * F1, F2, … Hovering or focusing a reference highlights its number on the screen when that
 * number is shown; otherwise a tooltip says the fact.
 */
import { useEffect, useState } from "react";
import { useLanguage } from "../i18n/language";
import type { RecapSentence } from "../lib/recap/compose";

export interface RecapFactView {
  id: string;
  /** The fact in words, for the button label and the tooltip. */
  label: string;
  /** Id of the element that shows the number, or null when it is not on this screen. */
  target: string | null;
}

const HIGHLIGHT = "recap-highlight";

function targetOf(fact: RecapFactView): HTMLElement | null {
  return fact.target ? document.getElementById(fact.target) : null;
}

export function Recap({
  id,
  title,
  sentences,
  facts,
}: {
  id: string;
  title: string;
  sentences: RecapSentence[];
  facts: RecapFactView[];
}) {
  const { tr } = useLanguage();
  const [tip, setTip] = useState<string | null>(null);
  const byId = new Map(facts.map((fact) => [fact.id, fact]));
  // Leaving the screen while a number is highlighted must not leave the highlight behind.
  useEffect(
    () => () => {
      for (const fact of facts) targetOf(fact)?.classList.remove(HIGHLIGHT);
    },
    [facts],
  );
  const show = (fact: RecapFactView) => {
    const target = targetOf(fact);
    if (target) target.classList.add(HIGHLIGHT);
    else setTip(fact.id);
  };
  const hide = (fact: RecapFactView) => {
    targetOf(fact)?.classList.remove(HIGHLIGHT);
    setTip((open) => (open === fact.id ? null : open));
  };
  const heading = `${id}-h`;
  return (
    <section className="section" aria-labelledby={heading}>
      <div className="card recap">
        <h2 id={heading} className="section-title">
          {title}
        </h2>
        <ol className="recap-list">
          {sentences.map((sentence) => (
            <li key={sentence.facts.join("-")}>
              {sentence.text}
              {sentence.facts.map((factId) => {
                const fact = byId.get(factId);
                if (!fact) return null;
                const tipId = `${id}-tip-${fact.id}`;
                return (
                  <span key={fact.id} className="recap-ref-wrap">
                    {" "}
                    <button
                      type="button"
                      className="recap-ref"
                      aria-label={tr("recapFact", { id: fact.id, label: fact.label })}
                      aria-describedby={tip === fact.id ? tipId : undefined}
                      onMouseEnter={() => show(fact)}
                      onMouseLeave={() => hide(fact)}
                      onFocus={() => show(fact)}
                      onBlur={() => hide(fact)}
                    >
                      {fact.id}
                    </button>
                    {tip === fact.id && (
                      <span id={tipId} role="tooltip" className="recap-tip">
                        {fact.label}
                      </span>
                    )}
                  </span>
                );
              })}
            </li>
          ))}
        </ol>
        <p className="muted recap-note">{tr("recapNote")}</p>
      </div>
    </section>
  );
}
