import { createContext, useCallback, useContext, useEffect, useMemo, useState } from "react";
import type { ReactNode } from "react";
import type { Language } from "../types";
import { t, type UiKey } from "./ui";

const STORAGE_KEY = "afterbattle-language";

/** The single source of the interface language (N8). */
export function storedLanguage(): Language {
  try {
    return localStorage.getItem(STORAGE_KEY) === "en" ? "en" : "pl";
  } catch {
    return "pl";
  }
}

interface LanguageValue {
  language: Language;
  setLanguage: (language: Language) => void;
  tr: (key: UiKey, params?: Record<string, string | number>) => string;
}

const LanguageContext = createContext<LanguageValue | null>(null);

export function LanguageProvider({
  children,
  initial,
}: {
  children: ReactNode;
  initial?: Language;
}) {
  const [language, setLanguageState] = useState<Language>(() => initial ?? storedLanguage());
  useEffect(() => {
    document.documentElement.lang = language;
  }, [language]);
  const setLanguage = useCallback((next: Language) => {
    try {
      localStorage.setItem(STORAGE_KEY, next);
    } catch {
      // Storage may be unavailable; the choice still applies for this run.
    }
    setLanguageState(next);
  }, []);
  const value = useMemo<LanguageValue>(
    () => ({ language, setLanguage, tr: (key, params) => t(language, key, params) }),
    [language, setLanguage],
  );
  return <LanguageContext.Provider value={value}>{children}</LanguageContext.Provider>;
}

export function useLanguage(): LanguageValue {
  const value = useContext(LanguageContext);
  if (!value) throw new Error("useLanguage outside LanguageProvider");
  return value;
}
