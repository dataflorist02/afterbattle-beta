import { describe, expect, it } from "vitest";
import { mergeRefreshedPage } from "./pages";

const row = (battleId: string) => ({ battleId });

describe("rows loaded with Show more across a refresh (spec 29 §2)", () => {
  it("puts the refreshed first page on top and keeps the rows loaded after it", () => {
    const loaded = ["c", "b", "a", "z", "y"].map(row);
    const fresh = ["d", "c", "b", "a"].map(row);
    expect(mergeRefreshedPage(fresh, loaded).map((item) => item.battleId)).toEqual([
      "d",
      "c",
      "b",
      "a",
      "z",
      "y",
    ]);
  });

  it("takes the refreshed version of a row that is in both", () => {
    const loaded = [{ battleId: "a", damage: 1 }];
    const fresh = [{ battleId: "a", damage: 2 }];
    expect(mergeRefreshedPage(fresh, loaded)).toEqual([{ battleId: "a", damage: 2 }]);
  });

  it("is the first page alone when nothing more was loaded", () => {
    const fresh = ["b", "a"].map(row);
    expect(mergeRefreshedPage(fresh, [])).toEqual(fresh);
    expect(mergeRefreshedPage([], ["a"].map(row))).toEqual(["a"].map(row));
  });
});
