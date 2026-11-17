import { createContext, useCallback, useContext, useEffect, useMemo, useState } from "react";
import type { ReactNode } from "react";
import { ADDRESSES, DEFAULT_ADDRESS, type Address } from "../lib/recap/address";
import type { Language } from "../types";
import { t, type UiKey } from "./ui";

const STORAGE_KEY = "afterbattle-language";
const ADDRESS_KEY = "afterbattle-address";

/** The single source of the interface language (N8). */
export function storedLanguage(): Language {
  try {
    return localStorage.getItem(STORAGE_KEY) === "en" ? "en" : "pl";
  } catch {
    return "pl";
  }
}

/** The stored choice, or null before the player answered (the beta 4 "impersonal" is neutral). */
function chosenAddress(): Address | null {
  try {
    const value = localStorage.getItem(ADDRESS_KEY);
    if (value === "impersonal") return "neutral";
    return ADDRESSES.find((address) => address === value) ?? null;
  } catch {
    return null;
  }
}

/** How the Polish texts address the player (spec 30 §2); neutral until chosen. */
export function storedAddress(): Address {
  return chosenAddress() ?? DEFAULT_ADDRESS;
}

interface LanguageValue {
  language: Language;
  setLanguage: (language: Language) => void;
  address: Address;
  /** Whether the player answered the address question (first run, decision 2026-11-16). */
  addressChosen: boolean;
  setAddress: (address: Address) => void;
  tr: (key: UiKey, params?: Record<string, string | number>) => string;
}

const LanguageContext = createContext<LanguageValue | null>(null);

function remember(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    // Storage may be unavailable; the choice still applies for this run.
  }
}

export function LanguageProvider({
  children,
  initial,
}: {
  children: ReactNode;
  initial?: Language;
}) {
  const [language, setLanguageState] = useState<Language>(() => initial ?? storedLanguage());
  const [address, setAddressState] = useState<Address>(storedAddress);
  const [addressChosen, setAddressChosen] = useState(() => chosenAddress() !== null);
  useEffect(() => {
    document.documentElement.lang = language;
  }, [language]);
  const setLanguage = useCallback((next: Language) => {
    remember(STORAGE_KEY, next);
    setLanguageState(next);
  }, []);
  const setAddress = useCallback((next: Address) => {
    remember(ADDRESS_KEY, next);
    setAddressState(next);
    setAddressChosen(true);
  }, []);
  const value = useMemo<LanguageValue>(
    () => ({
      language,
      setLanguage,
      address,
      addressChosen,
      setAddress,
      tr: (key, params) => t(language, key, params, address),
    }),
    [language, setLanguage, address, addressChosen, setAddress],
  );
  return <LanguageContext.Provider value={value}>{children}</LanguageContext.Provider>;
}

export function useLanguage(): LanguageValue {
  const value = useContext(LanguageContext);
  if (!value) throw new Error("useLanguage outside LanguageProvider");
  return value;
}
