//! `Workmanager`: one-off and periodic work by unique name, with the validation the
//! references apply. Work needs no configure; its events wait for a configured handler.

use std::sync::{Arc, OnceLock};

use crate::{
    backend::{Backend, CancelBy, WorkKind, WorkRequest, resolve_now},
    error::{BackgroundError, ErrorKind},
    options::{ExistingPeriodicWorkPolicy, ExistingWorkPolicy, WorkInfo},
};

/// The options of one piece of one-off work. Unset fields keep these defaults.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct OneOffOptions {
    /// The work's input data.
    pub input: crate::options::Input,
    /// How long to wait before the work runs. `None` runs at once.
    pub initial_delay: Option<std::time::Duration>,
    /// The conditions the work waits for.
    pub constraints: Option<crate::options::Constraints>,
    /// What a registration under a taken name does to the existing work. `None` keeps it.
    pub existing_work_policy: Option<ExistingWorkPolicy>,
    /// How a failed attempt waits before its next one.
    pub backoff_policy: Option<crate::options::BackoffPolicy>,
    /// The first backoff delay.
    pub backoff_policy_delay: Option<std::time::Duration>,
    /// A name to cancel the work by, besides its unique name.
    pub tag: Option<String>,
}

/// The options of one piece of periodic work. Unset fields keep these defaults.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PeriodicOptions {
    /// The period the work fires on. `None` is the 15-minute default.
    pub frequency: Option<std::time::Duration>,
    /// How far into a period the work may still fire.
    pub flex_interval: Option<std::time::Duration>,
    /// The work's input data.
    pub input: crate::options::Input,
    /// How long to wait before the first firing. `None` runs at once.
    pub initial_delay: Option<std::time::Duration>,
    /// The conditions the work waits for.
    pub constraints: Option<crate::options::Constraints>,
    /// What a registration under a taken name does to the existing work. `None` keeps it.
    pub existing_periodic_work_policy: Option<ExistingPeriodicWorkPolicy>,
    /// How a failed attempt waits before its next one.
    pub backoff_policy: Option<crate::options::BackoffPolicy>,
    /// The first backoff delay.
    pub backoff_policy_delay: Option<std::time::Duration>,
    /// A name to cancel the work by, besides its unique name.
    pub tag: Option<String>,
}

/// The work manager, as `workmanager` names it.
///
/// Clones share one manager. [`Workmanager::new`] returns the platform's shared handle, so
/// every handle in one app shares it too.
#[derive(Clone)]
pub struct Workmanager {
    backend: Arc<dyn Backend>,
}

impl Workmanager {
    /// Every name-taking call refuses an empty or whitespace name, as the scheduler's own
    /// id checks do.
    fn checked(name: &str) -> Result<(), BackgroundError> {
        if name.trim().is_empty() {
            Err(BackgroundError::new(ErrorKind::InvalidArgument))
        } else {
            Ok(())
        }
    }

    /// The platform's work manager over the process's one backend.
    pub fn new() -> Self {
        static PLATFORM: OnceLock<Workmanager> = OnceLock::new();
        PLATFORM
            .get_or_init(|| Self {
                backend: crate::platform::shared(),
            })
            .clone()
    }

    /// A work manager through `backend`. Tests and fakes use this.
    pub fn with_backend(backend: impl Backend + 'static) -> Self {
        Self {
            backend: Arc::new(backend),
        }
    }

    /// Registers work that fires once, `delay` after the call at the latest.
    pub fn register_one_off_task(
        &self,
        unique_name: &str,
        task_name: &str,
        options: OneOffOptions,
    ) -> Result<(), BackgroundError> {
        if unique_name.trim().is_empty() || task_name.trim().is_empty() {
            return Err(BackgroundError::new(ErrorKind::InvalidArgument));
        }
        let OneOffOptions {
            input,
            initial_delay,
            constraints,
            existing_work_policy,
            backoff_policy,
            backoff_policy_delay,
            tag,
        } = options;
        let request = WorkRequest {
            kind: WorkKind::OneOff,
            unique_name: unique_name.to_owned(),
            task_name: task_name.to_owned(),
            input,
            initial_delay,
            frequency: None,
            flex_interval: None,
            constraints,
            existing_work_policy,
            existing_periodic_work_policy: None,
            backoff_policy,
            backoff_policy_delay,
            tag,
        };
        resolve_now(self.backend.register(request))
    }

    /// Registers work that fires on every `frequency`, the 15-minute default when unset.
    pub fn register_periodic_task(
        &self,
        unique_name: &str,
        task_name: &str,
        options: PeriodicOptions,
    ) -> Result<(), BackgroundError> {
        if unique_name.trim().is_empty() || task_name.trim().is_empty() {
            return Err(BackgroundError::new(ErrorKind::InvalidArgument));
        }
        let PeriodicOptions {
            frequency,
            flex_interval,
            input,
            initial_delay,
            constraints,
            existing_periodic_work_policy,
            backoff_policy,
            backoff_policy_delay,
            tag,
        } = options;
        let request = WorkRequest {
            kind: WorkKind::Periodic,
            unique_name: unique_name.to_owned(),
            task_name: task_name.to_owned(),
            input,
            initial_delay,
            frequency,
            flex_interval,
            constraints,
            existing_work_policy: None,
            existing_periodic_work_policy,
            backoff_policy,
            backoff_policy_delay,
            tag,
        };
        resolve_now(self.backend.register(request))
    }

    /// Cancels the work under `unique_name`.
    pub fn cancel_by_unique_name(&self, unique_name: &str) -> Result<(), BackgroundError> {
        Self::checked(unique_name)?;
        resolve_now(
            self.backend
                .cancel(CancelBy::UniqueName(unique_name.to_owned())),
        )
    }

    /// Cancels every work registered with `tag`.
    pub fn cancel_by_tag(&self, tag: &str) -> Result<(), BackgroundError> {
        Self::checked(tag)?;
        resolve_now(self.backend.cancel(CancelBy::Tag(tag.to_owned())))
    }

    /// Cancels every piece of work.
    pub fn cancel_all(&self) -> Result<(), BackgroundError> {
        resolve_now(self.backend.cancel(CancelBy::All))
    }

    /// Whether the work under `unique_name` is scheduled or running.
    pub fn is_scheduled_by_unique_name(&self, unique_name: &str) -> Result<bool, BackgroundError> {
        Self::checked(unique_name)?;
        resolve_now(self.backend.is_scheduled(unique_name))
    }

    /// The work under `unique_name`, when it exists.
    pub fn get_work_info(&self, unique_name: &str) -> Result<Option<WorkInfo>, BackgroundError> {
        Self::checked(unique_name)?;
        resolve_now(self.backend.work_info(unique_name))
    }
}

impl Default for Workmanager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::{
        fake::FakeBackend,
        options::{BackoffPolicy, Constraints, InputValue, NetworkType, WorkState},
    };

    fn manager() -> (Workmanager, FakeBackend) {
        let backend = FakeBackend::new();
        let work = Workmanager::with_backend(backend.clone());
        (work, backend)
    }

    #[test]
    fn register_refuses_blank_names_without_touching_the_backend() {
        let (work, backend) = manager();
        for unique_name in ["", " "] {
            let error = work
                .register_one_off_task(unique_name, "sync", OneOffOptions::default())
                .unwrap_err();
            assert_eq!(error.code(), "ERR_BACKGROUND_INVALID_ARGUMENT");
        }
        let error = work
            .register_one_off_task("sync-work", "  ", OneOffOptions::default())
            .unwrap_err();
        assert_eq!(error.code(), "ERR_BACKGROUND_INVALID_ARGUMENT");
        assert_eq!(backend.registers(), 0);
    }

    #[test]
    fn register_one_off_builds_the_request() {
        let (work, backend) = manager();
        work.register_one_off_task(
            "sync-work",
            "sync",
            OneOffOptions {
                input: vec![("count".to_owned(), InputValue::Int(3))],
                initial_delay: Some(Duration::from_secs(30)),
                constraints: Some(Constraints {
                    required_network_type: Some(NetworkType::Unmetered),
                    requires_charging: true,
                    ..Constraints::default()
                }),
                existing_work_policy: Some(ExistingWorkPolicy::Replace),
                backoff_policy: Some(BackoffPolicy::Linear),
                backoff_policy_delay: Some(Duration::from_secs(10)),
                tag: Some("uploads".to_owned()),
            },
        )
        .unwrap();
        assert_eq!(backend.registers(), 1);
        let request = backend.last_request().unwrap();
        assert_eq!(request.kind, WorkKind::OneOff);
        assert_eq!(request.unique_name, "sync-work");
        assert_eq!(request.task_name, "sync");
        assert_eq!(
            request.input,
            vec![("count".to_owned(), InputValue::Int(3))]
        );
        assert_eq!(request.initial_delay, Some(Duration::from_secs(30)));
        assert_eq!(request.frequency, None);
        assert_eq!(request.flex_interval, None);
        assert_eq!(
            request.constraints,
            Some(Constraints {
                required_network_type: Some(NetworkType::Unmetered),
                requires_charging: true,
                ..Constraints::default()
            })
        );
        assert_eq!(
            request.existing_work_policy,
            Some(ExistingWorkPolicy::Replace)
        );
        assert_eq!(request.backoff_policy, Some(BackoffPolicy::Linear));
        assert_eq!(request.backoff_policy_delay, Some(Duration::from_secs(10)));
        assert_eq!(request.tag, Some("uploads".to_owned()));
    }

    #[test]
    fn register_periodic_builds_the_request() {
        let (work, backend) = manager();
        work.register_periodic_task(
            "sync-work",
            "sync",
            PeriodicOptions {
                frequency: Some(Duration::from_secs(30 * 60)),
                flex_interval: Some(Duration::from_secs(5 * 60)),
                existing_periodic_work_policy: Some(ExistingPeriodicWorkPolicy::Update),
                ..PeriodicOptions::default()
            },
        )
        .unwrap();
        let request = backend.last_request().unwrap();
        assert_eq!(request.kind, WorkKind::Periodic);
        assert_eq!(request.frequency, Some(Duration::from_secs(30 * 60)));
        assert_eq!(request.flex_interval, Some(Duration::from_secs(5 * 60)));
        assert_eq!(
            request.existing_periodic_work_policy,
            Some(ExistingPeriodicWorkPolicy::Update)
        );
        assert_eq!(request.existing_work_policy, None);
    }

    #[test]
    fn cancel_and_queries_go_to_the_backend() {
        let (work, backend) = manager();
        backend.set_is_scheduled(true);
        backend.set_work_info(Some(crate::options::WorkInfo {
            unique_name: "sync-work".to_owned(),
            state: WorkState::Running,
            is_periodic: false,
            task_name: Some("sync".to_owned()),
            tags: vec!["uploads".to_owned()],
        }));
        assert_eq!(backend.cancels(), 0);

        work.cancel_by_unique_name("sync-work").unwrap();
        work.cancel_by_tag("uploads").unwrap();
        work.cancel_all().unwrap();
        assert_eq!(backend.cancels(), 3);

        assert!(work.is_scheduled_by_unique_name("sync-work").unwrap());
        let info = work.get_work_info("sync-work").unwrap().unwrap();
        assert_eq!(info.state, WorkState::Running);
        assert_eq!(info.tags, vec!["uploads".to_owned()]);
    }

    #[test]
    fn name_taking_calls_refuse_blank_names_without_touching_the_backend() {
        let (work, backend) = manager();
        for name in ["", "  "] {
            assert_eq!(
                work.cancel_by_unique_name(name).unwrap_err().code(),
                "ERR_BACKGROUND_INVALID_ARGUMENT"
            );
            assert_eq!(
                work.cancel_by_tag(name).unwrap_err().code(),
                "ERR_BACKGROUND_INVALID_ARGUMENT"
            );
            assert_eq!(
                work.is_scheduled_by_unique_name(name).unwrap_err().code(),
                "ERR_BACKGROUND_INVALID_ARGUMENT"
            );
            assert_eq!(
                work.get_work_info(name).unwrap_err().code(),
                "ERR_BACKGROUND_INVALID_ARGUMENT"
            );
        }
        assert_eq!(backend.cancels(), 0);
    }
}
