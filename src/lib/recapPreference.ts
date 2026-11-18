/**
 * Spec 31 §3: whether summary cards show all sentences. One preference for the Session, Battle
 * and Goals screens, closed by default; kept like the language and the address form.
 */
const KEY = "afterbattle-recap-expanded";

export function storedRecapExpanded(): boolean {
  try {
    return localStorage.getItem(KEY) === "true";
  } catch {
    return false;
  }
}

export function rememberRecapExpanded(expanded: boolean): void {
  try {
    localStorage.setItem(KEY, String(expanded));
  } catch {
    // Storage may be unavailable; the choice still applies until the card is left.
  }
}
