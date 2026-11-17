/**
 * Polish numeral agreement from `Intl.PluralRules` (spec 30 §5): one for 1, few for 2–4 (not
 * 12–14, also 22–24), many for 0 and 5+, other for fractions ("6,4 punktu procentowego").
 */

/** The forms after 1, after 2–4 and after 0 or 5+ ("pojazd", "pojazdy", "pojazdów"). */
export type Forms = readonly [one: string, few: string, many: string];

const RULES = new Intl.PluralRules("pl-PL");

export function plural(count: number, forms: Forms, fraction: string = forms[1]): string {
  switch (RULES.select(count)) {
    case "one":
      return forms[0];
    case "few":
      return forms[1];
    case "many":
      return forms[2];
    default:
      return fraction;
  }
}
