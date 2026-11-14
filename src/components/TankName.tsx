/**
 * Vehicle names (E5-ui §2): the game's own name when its translation files are readable, else the
 * stored code made readable ("czech-Cz21_Vz_60S_Dravec" → "Vz 60S Dravec"). Names live only in
 * memory and on screen. Lists use the short name; the full name is in the title and the header.
 */
import { createContext, useContext, useEffect, useState } from "react";
import type { ReactNode } from "react";
import { api } from "../api";
import { useLanguage } from "../i18n/language";
import { vehicleDisplayName } from "../lib/presentation";
import type { Language, VehicleNameView } from "../types";

type Names = Partial<Record<string, VehicleNameView>>;

const NamesContext = createContext<Names>({});

export function VehicleNamesProvider({
  children,
  refreshKey,
}: {
  children: ReactNode;
  /** Any value that changes when the game folder may have changed. */
  refreshKey?: unknown;
}) {
  const { language } = useLanguage();
  const [names, setNames] = useState<Names>({});
  useEffect(() => {
    let live = true;
    Promise.resolve()
      .then(() => api.vehicleNames(language))
      .then(
        (view) => {
          if (live) setNames(view?.names ?? {});
        },
        () => {
          if (live) setNames({});
        },
      );
    return () => {
      live = false;
    };
  }, [language, refreshKey]);
  return <NamesContext.Provider value={names}>{children}</NamesContext.Provider>;
}

export interface TankLabel {
  full: string;
  short: string;
}

export function tankLabel(
  names: Names,
  vehicleKey: string | null | undefined,
  tankName: string | null | undefined,
  language: Language,
): TankLabel {
  const known = vehicleKey ? names[vehicleKey.toLowerCase()] : undefined;
  if (known) return { full: known.name, short: known.shortName ?? known.name };
  const readable = vehicleDisplayName({ tankName, vehicleKey }, language);
  return { full: readable, short: readable };
}

export function useTankLabel() {
  const names = useContext(NamesContext);
  const { language } = useLanguage();
  return (vehicleKey: string | null | undefined, tankName: string | null | undefined) =>
    tankLabel(names, vehicleKey, tankName, language);
}

/** One line, ellipsis when too long, the full name always in the title. */
export function TankName({
  vehicleKey,
  tankName,
  full = false,
}: {
  vehicleKey: string | null | undefined;
  tankName: string | null | undefined;
  full?: boolean;
}) {
  const label = useTankLabel()(vehicleKey, tankName);
  return (
    <span className="truncate" title={label.full}>
      {full ? label.full : label.short}
    </span>
  );
}
