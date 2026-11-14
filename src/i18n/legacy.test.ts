import { describe, expect, it } from "vitest";
import { feedbackText, translate } from "./legacy";
describe("translations", () => {
  it("supports both languages", () => {
    expect(translate("pl", "session")).toBe("Sesja");
    expect(translate("en", "session")).toBe("Session");
  });
  it("localizes all vehicle classes and catalog controls", () => {
    expect(translate("pl", "vehicleClassLight")).toBe("Czołg lekki");
    expect(translate("pl", "vehicleClassTankDestroyer")).toBe("Niszczyciel czołgów");
    expect(translate("pl", "vehicleClassUnknown")).toBe("Nieznana klasa");
    expect(translate("en", "vehicleClassHeavy")).toBe("Heavy tank");
    expect(translate("en", "refreshVehicleClasses")).toBe("Refresh vehicle class data");
  });
  it("formats feedback without leaking identifiers", () => {
    expect(
      feedbackText("en", {
        code: "damage_above_baseline",
        tone: "positive",
        confidence: "medium",
        actual: 2000,
        baseline: 1000,
        sampleSize: 10,
      }),
    ).toContain("2,000");
  });
});
