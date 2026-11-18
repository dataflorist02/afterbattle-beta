/**
 * Spec 32 §4.1: where the app opens. A running session, or one that ended within the inactivity
 * limit, opens Session; otherwise Overview. A choice in the navigation wins until the app closes
 * (App keeps it); there is no "start screen" setting in 1.0.0.
 */
import type { AppSnapshot } from "../types";

export type StartPage = "session" | "overview";

export function startPage(snapshot: AppSnapshot, now: number): StartPage {
  // The first start (no game folder yet) is asked on Session.
  if (!snapshot.configured || snapshot.activeSession) return "session";
  const limit = snapshot.inactivityMinutes * 60_000;
  const recent = snapshot.recentSessions.some((session) => {
    const end = session.endedAt ?? session.lastBattleAt;
    return end !== null && now - end <= limit;
  });
  return recent ? "session" : "overview";
}
