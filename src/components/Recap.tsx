/**
 * A template summary card (spec 30 §8): the sentences, each followed by references F1, F2, …
 * Hovering or focusing a reference highlights its number on the screen when that number is shown;
 * otherwise a tooltip says the fact. Spec 31 §3: the first sentence always shows, the rest opens
 * with "Pokaż więcej (N)"; the choice is one preference for every screen, closed by default.
 */
import { useEffect, useState } from "react";
import { useLanguage } from "../i18n/language";
import type { RecapSentence } from "../lib/recap/compose";
import { rememberRecapExpanded, storedRecapExpanded } from "../lib/recapPreference";

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

function Chevron() {
  return (
    <svg
      aria-hidden="true"
      width="16"
      height="16"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <polyline points="6 9 12 15 18 9" />
    </svg>
  );
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
  const [expanded, setExpanded] = useState(storedRecapExpanded);
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
  const [first, ...rest] = sentences;
  const toggle = () => {
    const next = !expanded;
    if (!next) {
      // Closing hides the later sentences: their highlights and tooltips go with them.
      const shown = new Set(first?.facts ?? []);
      for (const sentence of rest)
        for (const factId of sentence.facts) {
          if (shown.has(factId)) continue;
          const fact = byId.get(factId);
          if (fact) hide(fact);
        }
    }
    setExpanded(next);
    rememberRecapExpanded(next);
  };
  const references = (sentence: RecapSentence) =>
    sentence.facts.map((factId) => {
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
    });
  if (!first) return null;
  const heading = `${id}-h`;
  const restId = `${id}-rest`;
  const note = <p className="muted recap-note">{tr("recapNote")}</p>;
  return (
    <section className="section" aria-labelledby={heading}>
      <div className="card recap">
        <div className="recap-head">
          <div className="recap-lead">
            <h2 id={heading} className="recap-title">
              {title}
            </h2>
            <p className="recap-first">
              {first.text}
              {references(first)}
            </p>
          </div>
          {rest.length > 0 && (
            <button
              type="button"
              className="recap-more"
              aria-expanded={expanded}
              aria-controls={restId}
              onClick={toggle}
            >
              {expanded ? tr("recapLess") : tr("recapMore", { count: rest.length })}
              <Chevron />
            </button>
          )}
        </div>
        {rest.length > 0 ? (
          <div
            id={restId}
            className="recap-rest"
            data-open={expanded}
            inert={!expanded}
            aria-hidden={expanded ? undefined : true}
          >
            <div className="recap-rest-inner">
              <ol className="recap-list">
                {rest.map((sentence) => (
                  <li key={sentence.facts.join("-") || sentence.text}>
                    {sentence.text}
                    {references(sentence)}
                  </li>
                ))}
              </ol>
              {note}
            </div>
          </div>
        ) : (
          note
        )}
      </div>
    </section>
  );
}
