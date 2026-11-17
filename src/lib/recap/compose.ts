/**
 * Putting a summary together (spec 30 §5, §6): each sentence has 2–4 variants with the same
 * meaning; the variant follows the seed from Rust, the sentence's slot and its place in the text,
 * so the same screen always shows the same text. No two sentences start with the same word.
 */
import { applyAddress, fitsAddress, type Address } from "./address";

/** One sentence of a summary with the ids of the facts it says ("F1"). */
export interface RecapSentence {
  text: string;
  facts: string[];
}

/** A planned sentence: its slot name, the facts it cites and every variant of its text. */
export interface SentencePlan {
  slot: string;
  facts: string[];
  /** Variants without the capital letter and the full stop; verbs as `{~stem}` markers. */
  variants: string[];
}

/** FNV-1a, 32 bits; the same function as `recap::seed` in Rust. */
export function fnv1a(text: string): number {
  let hash = 0x811c9dc5;
  for (const byte of new TextEncoder().encode(text)) {
    hash ^= byte;
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  return hash >>> 0;
}

/**
 * The first variant tried for a slot at a place in the text. The FNV hash is mixed (MurmurHash3
 * fmix32) first: its lowest bits alone depend only on the parity of the bytes, so "% 2" would
 * hardly change between seeds.
 */
export function variantIndex(seed: number, slot: string, position: number, count: number): number {
  let hash = fnv1a(`${seed}:${slot}:${position}`);
  hash ^= hash >>> 16;
  hash = Math.imul(hash, 0x85ebca6b);
  hash ^= hash >>> 13;
  hash = Math.imul(hash, 0xc2b2ae35);
  hash ^= hash >>> 16;
  return (hash >>> 0) % count;
}

/** The variants of a plan this form can say; the neutral form skips those with a verb. */
export function variantsFor(plan: SentencePlan, address: Address): string[] {
  const usable = plan.variants.filter((variant) => fitsAddress(variant, address));
  if (usable.length === 0) throw new Error(`No ${address} variant for the slot "${plan.slot}"`);
  return usable;
}

/** A finished sentence: verbs in the chosen form, capital letter, full stop. */
export function finish(variant: string, address: Address): string {
  const text = applyAddress(variant, address);
  return `${text[0].toLocaleUpperCase("pl-PL")}${text.slice(1)}.`;
}

function firstWord(text: string): string {
  return text.split(" ")[0].toLocaleLowerCase("pl-PL");
}

/**
 * Picks one variant per plan. When the chosen variant would start like an earlier sentence, the
 * next one is taken; when every variant would, the first choice stays.
 */
export function chooseSentences(
  seed: number,
  plans: SentencePlan[],
  address: Address,
): RecapSentence[] {
  const starts = new Set<string>();
  return plans.map((plan, position) => {
    const variants = variantsFor(plan, address);
    const count = variants.length;
    const first = variantIndex(seed, plan.slot, position, count);
    let chosen = finish(variants[first], address);
    for (let step = 0; step < count; step++) {
      const candidate = finish(variants[(first + step) % count], address);
      if (!starts.has(firstWord(candidate))) {
        chosen = candidate;
        break;
      }
    }
    starts.add(firstWord(chosen));
    return { text: chosen, facts: plan.facts };
  });
}
