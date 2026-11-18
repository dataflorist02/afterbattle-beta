import { describe, expect, it } from "vitest";
import * as fx from "../test/fixtures";
import { startPage } from "./start";

const MINUTE = 60_000;

describe("start screen (spec 32 §4.1)", () => {
  const now = fx.NOW + 10 * 60 * MINUTE;

  it("opens Session during a session", () => {
    expect(startPage(fx.snapshot(), now)).toBe("session");
  });

  it("opens Session for a session ended within the inactivity limit, Overview after it", () => {
    const ended = (minutesAgo: number) =>
      fx.snapshot({
        activeSession: null,
        inactivityMinutes: 60,
        recentSessions: [
          fx.session({
            status: "timed_out",
            endedAt: now - minutesAgo * MINUTE,
            lastBattleAt: now - (minutesAgo + 60) * MINUTE,
          }),
        ],
      });
    expect(startPage(ended(59), now)).toBe("session");
    expect(startPage(ended(60), now)).toBe("session");
    expect(startPage(ended(61), now)).toBe("overview");
  });

  it("opens Session for the first start, where the game folder is asked", () => {
    expect(
      startPage(fx.snapshot({ configured: false, activeSession: null, recentSessions: [] }), now),
    ).toBe("session");
  });

  it("opens Overview without any session", () => {
    expect(startPage(fx.snapshot({ activeSession: null, recentSessions: [] }), now)).toBe(
      "overview",
    );
  });
});
