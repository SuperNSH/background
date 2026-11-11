//! The errors a call reports. Each has a stable code; a message says what to do.

use std::fmt;

/// What went wrong, as a stable code; see [`BackgroundError::code`].
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    /// Background execution does not run on this target, the web included.
    NotAvailable,
    /// The scheduler has no configured handler yet; `start` and `schedule_task` need one.
    NotConfigured,
    /// An argument is empty or not allowed on this platform, such as a task id of only spaces.
    InvalidArgument,
    /// The platform's background scheduler failed; `status` holds its own code where it gives one.
    Backend,
}

/// A failed scheduler or work call.
///
/// `Display` names the code and the message. Neither holds app data.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{}", .kind.message())]
pub struct BackgroundError {
    kind: ErrorKind,
    status: Option<i64>,
}

impl BackgroundError {
    /// An error of `kind`, with no platform status.
    pub fn new(kind: ErrorKind) -> Self {
        Self { kind, status: None }
    }

    /// The same error, carrying the platform's own status code.
    #[must_use]
    pub fn with_status(mut self, status: i64) -> Self {
        self.status = Some(status);
        self
    }

    /// What went wrong.
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// The platform's status code, when the platform gave one.
    pub fn status(&self) -> Option<i64> {
        self.status
    }

    /// The stable code, such as `ERR_BACKGROUND_NOT_CONFIGURED`. Apps match on this.
    pub fn code(&self) -> &'static str {
        self.kind.code()
    }
}

impl ErrorKind {
    /// The stable code of this kind.
    pub fn code(self) -> &'static str {
        match self {
            ErrorKind::NotAvailable => "ERR_BACKGROUND_NOT_AVAILABLE",
            ErrorKind::NotConfigured => "ERR_BACKGROUND_NOT_CONFIGURED",
            ErrorKind::InvalidArgument => "ERR_BACKGROUND_INVALID_ARGUMENT",
            ErrorKind::Backend => "ERR_BACKGROUND_BACKEND",
        }
    }

    /// What to do about it, in one sentence.
    pub fn message(self) -> &'static str {
        match self {
            ErrorKind::NotAvailable => "Background execution is not available on this target.",
            ErrorKind::NotConfigured => {
                "Configure the scheduler with a task handler before starting it."
            }
            ErrorKind::InvalidArgument => "An argument is empty or not allowed on this platform.",
            ErrorKind::Backend => "The platform's background scheduler failed.",
        }
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_of_codes_and_messages_is_pinned() {
        let table = [
            (
                ErrorKind::NotAvailable,
                "ERR_BACKGROUND_NOT_AVAILABLE",
                "Background execution is not available on this target.",
            ),
            (
                ErrorKind::NotConfigured,
                "ERR_BACKGROUND_NOT_CONFIGURED",
                "Configure the scheduler with a task handler before starting it.",
            ),
            (
                ErrorKind::InvalidArgument,
                "ERR_BACKGROUND_INVALID_ARGUMENT",
                "An argument is empty or not allowed on this platform.",
            ),
            (
                ErrorKind::Backend,
                "ERR_BACKGROUND_BACKEND",
                "The platform's background scheduler failed.",
            ),
        ];
        for (kind, code, message) in table {
            assert_eq!(kind.code(), code);
            assert_eq!(kind.message(), message);
            assert_eq!(kind.to_string(), code);
        }
    }

    #[test]
    fn an_error_prints_its_message_and_carries_its_code() {
        let error = BackgroundError::new(ErrorKind::NotConfigured);
        assert_eq!(
            error.to_string(),
            "Configure the scheduler with a task handler before starting it."
        );
        assert_eq!(error.code(), "ERR_BACKGROUND_NOT_CONFIGURED");
        assert_eq!(error.kind(), ErrorKind::NotConfigured);
        assert_eq!(error.status(), None);
    }

    #[test]
    fn with_status_keeps_the_kind_and_carries_the_status() {
        let error = BackgroundError::new(ErrorKind::Backend).with_status(7);
        assert_eq!(error.kind(), ErrorKind::Backend);
        assert_eq!(error.status(), Some(7));
        assert_eq!(error.code(), "ERR_BACKGROUND_BACKEND");
    }
}
