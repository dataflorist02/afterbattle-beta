import { readFileSync } from "node:fs";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { LanguageProvider } from "../i18n/language";

const controls = vi.hoisted(() => ({
  minimizeWindow: vi.fn(),
  toggleMaximizeWindow: vi.fn(),
  closeWindow: vi.fn(),
}));
vi.mock("./windowControls", async (original) => ({
  ...(await original<typeof import("./windowControls")>()),
  ...controls,
}));

const { TitleBar } = await import("./TitleBar");
const { isMaximized } = await import("./windowControls");

function setWindow(outerWidth: number, outerHeight: number) {
  Object.defineProperty(window, "outerWidth", { value: outerWidth, configurable: true });
  Object.defineProperty(window, "outerHeight", { value: outerHeight, configurable: true });
  Object.defineProperty(window.screen, "availWidth", { value: 2560, configurable: true });
  Object.defineProperty(window.screen, "availHeight", { value: 1392, configurable: true });
}

function renderBar(language: "pl" | "en" = "pl") {
  return render(
    <LanguageProvider initial={language}>
      <TitleBar />
    </LanguageProvider>,
  );
}

beforeEach(() => {
  setWindow(1240, 800);
  for (const mock of Object.values(controls)) mock.mockReset();
});
afterEach(cleanup);

describe("title bar (E5-maps-hd §6)", () => {
  it("shows the 16 px app icon before the title, part of the drag area (spec 29 §4)", () => {
    const { container } = renderBar();
    const title = container.querySelector(".titlebar-title")!;
    const icon = title.querySelector("svg")!;
    expect(icon).toHaveAttribute("width", "16");
    expect(icon).toHaveAttribute("aria-hidden", "true");
    expect(title.firstElementChild).toBe(icon);
    expect(title.querySelector("rect")).not.toBeNull();
    expect(title.querySelector("defs")).toBeNull();
    expect(title).toHaveAttribute("data-tauri-drag-region");
    // Tauri drags only when the pressed element has the attribute: the icon lets clicks through.
    expect(icon).toHaveClass("titlebar-icon");
    expect(readFileSync("src/styles/base.css", "utf8")).toMatch(
      /\.titlebar-icon\s*\{[^}]*pointer-events:\s*none;/,
    );
  });

  it("drags from the whole bar except the buttons, which have names and keyboard focus", () => {
    renderBar();
    const bar = screen.getByRole("banner");
    expect(bar).toHaveAttribute("data-tauri-drag-region");
    expect(bar).toHaveTextContent("Afterbattle beta 3");
    const buttons = screen.getAllByRole("button");
    expect(buttons.map((button) => button.getAttribute("aria-label"))).toEqual([
      "Minimalizuj",
      "Maksymalizuj",
      "Zamknij do zasobnika",
    ]);
    for (const button of buttons) {
      expect(button).not.toHaveAttribute("data-tauri-drag-region");
      expect(button.tabIndex).toBe(0);
      expect(button).toHaveAttribute("type", "button");
    }
  });

  it("calls the window for each button; closing goes the same way as Alt+F4 (to the tray)", () => {
    renderBar();
    fireEvent.click(screen.getByRole("button", { name: "Minimalizuj" }));
    fireEvent.click(screen.getByRole("button", { name: "Maksymalizuj" }));
    fireEvent.click(screen.getByRole("button", { name: "Zamknij do zasobnika" }));
    expect(controls.minimizeWindow).toHaveBeenCalledTimes(1);
    expect(controls.toggleMaximizeWindow).toHaveBeenCalledTimes(1);
    expect(controls.closeWindow).toHaveBeenCalledTimes(1);
  });

  it("shows restore while maximized and follows resizing", () => {
    renderBar("en");
    expect(screen.getByRole("button", { name: "Maximize" })).toHaveAttribute(
      "data-state",
      "normal",
    );
    setWindow(2560, 1392);
    act(() => {
      window.dispatchEvent(new Event("resize"));
    });
    expect(screen.getByRole("button", { name: "Restore" })).toHaveAttribute(
      "data-state",
      "maximized",
    );
    expect(screen.getByRole("button", { name: "Minimize" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Close to the tray" })).toBeInTheDocument();
  });

  it("treats a window filling the work area as maximized", () => {
    setWindow(2560, 1392);
    expect(isMaximized()).toBe(true);
    setWindow(2559, 1392);
    expect(isMaximized()).toBe(false);
    setWindow(2560, 1391);
    expect(isMaximized()).toBe(false);
  });
});

describe("window configuration (E5-maps-hd §6)", () => {
  it("has no system frame, keeps the shadow and resizing", () => {
    const config = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8"));
    const [main] = config.app.windows;
    expect(main.decorations).toBe(false);
    expect(main.shadow).toBe(true);
    expect(main.resizable).toBe(true);
  });

  it("grants only the title bar's window calls, listening to backend events and the folder dialog", () => {
    const capability = JSON.parse(readFileSync("src-tauri/capabilities/default.json", "utf8"));
    expect(capability.windows).toEqual(["main"]);
    expect([...capability.permissions].sort()).toEqual([
      "core:event:allow-listen",
      "core:event:allow-unlisten",
      "core:window:allow-close",
      "core:window:allow-internal-toggle-maximize",
      "core:window:allow-minimize",
      "core:window:allow-start-dragging",
      "core:window:allow-toggle-maximize",
      "dialog:allow-open",
    ]);
  });
});
