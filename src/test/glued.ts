import { expect } from "vitest";

const INLINE = new Set(["SPAN", "B", "STRONG", "SMALL", "I", "EM", "A", "LABEL", "ABBR"]);
const LETTER = "A-Za-zĄĆĘŁŃÓŚŹŻąćęłńóśźż";
const GLUED = new RegExp(`[${LETTER}]\\d|\\d[${LETTER}]`);

/**
 * E5-ui §3: no rendered line may glue a number to a label ("Strzały57", "Ewidentne0"). A line is an
 * element whose children are all inline, so its text renders without breaks between them (buttons
 * are separate boxes, so a row of segments is not a line).
 */
export function expectNoGluedNumbers(root: HTMLElement) {
  const lines = [root, ...Array.from(root.querySelectorAll<HTMLElement>("*"))].filter(
    (element) =>
      Array.from(element.children).every((child) => INLINE.has(child.tagName)) &&
      (element.textContent ?? "").trim() !== "",
  );
  expect(lines.length).toBeGreaterThan(0);
  for (const line of lines) {
    // "WN8" is a name, not a number glued to a label.
    const text = (line.textContent ?? "").replace(/\s+/g, " ").replace(/WN8/g, "WN");
    expect(GLUED.test(text), `glued number and label: "${text}"`).toBe(false);
  }
}
