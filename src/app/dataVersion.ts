/**
 * The data version (spec 29 §2): one number for the whole window that grows each time the
 * backend reports changed data and the new snapshot is in. Pages reload with it in place
 * (`useAsync` `refreshKey`), keeping their data, chosen tank, filters and scroll.
 */
import { createContext, useContext } from "react";

export const DataVersionContext = createContext(0);

export function useDataVersion(): number {
  return useContext(DataVersionContext);
}
