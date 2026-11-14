import { useState } from "react";
import { api } from "../api";
import { useLanguage } from "../i18n/language";
import { formatNumber } from "../lib/format";
import type { ImportProgressEvent } from "../types";
import { ErrorState } from "./ui";

/** "Reading battle files: x of y" with a stop button (E3, spec 25 §5.1). */
export function ImportBar({ progress }: { progress: Pick<ImportProgressEvent, "done" | "total"> }) {
  const { language, tr } = useLanguage();
  const [error, setError] = useState<string | undefined | null>(null);
  const percent = progress.total > 0 ? Math.round((progress.done / progress.total) * 100) : 0;
  return (
    <div className="card section">
      <div className="progress" role="status">
        <span>
          {tr("importProgress", {
            done: formatNumber(progress.done, language),
            total: formatNumber(progress.total, language),
          })}
        </span>
        <div className="track" role="img" aria-label={`${percent}%`}>
          <div style={{ width: `${percent}%` }} />
        </div>
        <button
          type="button"
          className="button"
          onClick={() => {
            setError(null);
            api.cancelImport().catch((failure: { code?: string }) => setError(failure?.code));
          }}
        >
          {tr("importStop")}
        </button>
      </div>
      {error !== null && <ErrorState code={error} />}
    </div>
  );
}
