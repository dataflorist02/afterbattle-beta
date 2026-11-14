//! Backend → frontend events (spec 20, D6, E3): `import-progress`, `data-changed`,
//! `session-ended`. The frontend listens instead of polling every 10 s.

use std::sync::Arc;

use serde::Serialize;

pub(crate) const IMPORT_PROGRESS: &str = "import-progress";
pub(crate) const DATA_CHANGED: &str = "data-changed";
pub(crate) const SESSION_ENDED: &str = "session-ended";

/// One event for the frontend.
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum AppEvent {
    /// Full import progress; `cancelled` is set on the last event of a cancelled run.
    ImportProgress {
        done: usize,
        total: usize,
        finished: bool,
        cancelled: bool,
    },
    /// Battles, sessions or challenges changed; the frontend reloads what it shows.
    DataChanged,
    /// A session ended (by the inactivity limit or by the player).
    SessionEnded { session_id: String },
}

impl AppEvent {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::ImportProgress { .. } => IMPORT_PROGRESS,
            Self::DataChanged => DATA_CHANGED,
            Self::SessionEnded { .. } => SESSION_ENDED,
        }
    }

    /// The JSON payload sent with the event.
    pub(crate) fn payload(&self) -> serde_json::Value {
        match self {
            Self::ImportProgress {
                done,
                total,
                finished,
                cancelled,
            } => serde_json::json!({
                "done": done, "total": total, "finished": finished, "cancelled": cancelled
            }),
            Self::DataChanged => serde_json::json!({}),
            Self::SessionEnded { session_id } => serde_json::json!({ "sessionId": session_id }),
        }
    }
}

/// Where events go: the Tauri app handle in the application, a recorder in tests.
pub(crate) trait EventSink: Send + Sync + std::fmt::Debug {
    fn emit(&self, event: AppEvent);
}

/// Drops every event (recovery mode, tests that do not look at events).
#[derive(Debug, Default)]
pub(crate) struct NoEvents;

impl EventSink for NoEvents {
    fn emit(&self, _: AppEvent) {}
}

/// Records events for tests.
#[cfg(test)]
#[derive(Debug, Default, Clone)]
pub(crate) struct RecordedEvents(pub Arc<std::sync::Mutex<Vec<AppEvent>>>);

#[cfg(test)]
impl EventSink for RecordedEvents {
    fn emit(&self, event: AppEvent) {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(event);
    }
}

#[cfg(test)]
impl RecordedEvents {
    pub(crate) fn take(&self) -> Vec<AppEvent> {
        std::mem::take(
            &mut *self
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        )
    }
}

pub(crate) fn no_events() -> Arc<dyn EventSink> {
    Arc::new(NoEvents)
}
