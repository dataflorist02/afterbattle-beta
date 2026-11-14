//! Injected clock (audit T8, E3): tests move time forward instead of sleeping.

use std::{sync::Arc, time::SystemTime};

pub(crate) trait Clock: Send + Sync + std::fmt::Debug {
    fn now(&self) -> SystemTime;

    fn now_ms(&self) -> i64 {
        self.now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .ok()
            .and_then(|duration| i64::try_from(duration.as_millis()).ok())
            .unwrap_or(0)
    }
}

#[derive(Debug, Default)]
pub(crate) struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> SystemTime {
        SystemTime::now()
    }
}

pub(crate) fn system_clock() -> Arc<dyn Clock> {
    Arc::new(SystemClock)
}

/// A clock that moves only when told to.
#[cfg(test)]
#[derive(Debug, Clone)]
pub(crate) struct ManualClock(Arc<std::sync::Mutex<SystemTime>>);

#[cfg(test)]
impl ManualClock {
    pub(crate) fn starting_now() -> Self {
        Self(Arc::new(std::sync::Mutex::new(SystemTime::now())))
    }

    pub(crate) fn advance(&self, by: std::time::Duration) {
        let mut now = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *now += by;
    }
}

#[cfg(test)]
impl Clock for ManualClock {
    fn now(&self) -> SystemTime {
        *self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
