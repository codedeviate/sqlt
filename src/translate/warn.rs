//! Translation warnings — emitted whenever the rewriter has to drop or
//! transform a construct that has no faithful equivalent in the target
//! dialect.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WarnCode {
    /// `RETURNING` was dropped because the target dialect does not support it.
    ReturningDropped,
    /// `CREATE SEQUENCE` was dropped (or kept but flagged) for a target that
    /// does not support it.
    SequenceDropped,
    /// `ON DUPLICATE KEY UPDATE` was kept verbatim and could not be rewritten
    /// to the target's equivalent (e.g. `ON CONFLICT ... DO UPDATE`).
    OnDuplicateKeyUnsupported,
    /// A MariaDB raw-fallback fragment was kept verbatim because the target
    /// dialect cannot represent it. The emitted SQL will likely fail to
    /// execute against the target server.
    RawPassthrough,
}

impl WarnCode {
    pub fn as_str(self) -> &'static str {
        match self {
            WarnCode::ReturningDropped => "RETURNING_DROPPED",
            WarnCode::SequenceDropped => "SEQUENCE_DROPPED",
            WarnCode::OnDuplicateKeyUnsupported => "ON_DUPLICATE_KEY_UNSUPPORTED",
            WarnCode::RawPassthrough => "RAW_PASSTHROUGH",
        }
    }
}

impl fmt::Display for WarnCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct Warning {
    pub code: WarnCode,
    pub message: String,
}

impl Warning {
    pub fn new(code: WarnCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "warning: {} {}", self.code, self.message)
    }
}

pub trait WarnSink {
    fn warn(&mut self, w: Warning);
    fn count(&self) -> usize;
}

pub struct StderrSink {
    n: usize,
}

impl Default for StderrSink {
    fn default() -> Self {
        Self::new()
    }
}

impl StderrSink {
    pub fn new() -> Self {
        Self { n: 0 }
    }
}

impl WarnSink for StderrSink {
    fn warn(&mut self, w: Warning) {
        self.n += 1;
        eprintln!("{w}");
    }
    fn count(&self) -> usize {
        self.n
    }
}

#[derive(Debug, Default)]
pub struct CollectingSink {
    pub items: Vec<Warning>,
}

impl WarnSink for CollectingSink {
    fn warn(&mut self, w: Warning) {
        self.items.push(w);
    }
    fn count(&self) -> usize {
        self.items.len()
    }
}
