//! The scheduler and work manager as TSX sees them: `NativeBackgroundFetch`, a class whose
//! async members reject with a `code` on failure. Events stay on the Rust side until an NSH
//! bridge delivers them (see `docs/parity.md`).

use std::time::Duration;

use nsh_core::prelude::*;

use crate::{
    BackgroundError, BackgroundFetch, BackoffPolicy, Constraints, ErrorKind,
    ExistingPeriodicWorkPolicy, ExistingWorkPolicy, FetchConfig, NetworkType, OneOffOptions,
    PeriodicOptions, TaskConfig, TaskType, WorkInfo, WorkState, Workmanager,
};

/// The fetch settings, from TSX. Unset fields keep the defaults; minutes become seconds.
#[derive(JsObject, Clone, Debug, Default)]
pub struct NativeFetchConfig {
    /// The shortest interval between two fetch events, in minutes. 15 by default.
    pub minimum_fetch_interval_minutes: Option<f64>,
    /// Keep the schedule when the app terminates. Android only.
    pub stop_on_terminate: Option<bool>,
    /// Start the schedule at boot. Android only.
    pub start_on_boot: Option<bool>,
    /// Run the handler when the app is not running. Android only.
    pub enable_headless: Option<bool>,
    /// Schedule with AlarmManager, not JobScheduler. Android only.
    pub force_alarm_manager: Option<bool>,
    /// `"none"`, `"any"`, `"unmetered"`, `"notRoaming"` or `"cellular"`.
    pub required_network_type: Option<String>,
    /// Refuse to run when the battery is low.
    pub requires_battery_not_low: Option<bool>,
    /// Refuse to run when storage is low.
    pub requires_storage_not_low: Option<bool>,
    /// Refuse to run unless the device is charging.
    pub requires_charging: Option<bool>,
    /// Refuse to run unless the device is idle.
    pub requires_device_idle: Option<bool>,
}

/// A task to schedule, from TSX. `taskId` is required; the rest keeps its defaults.
#[derive(JsObject, Clone, Debug, Default)]
pub struct NativeTaskConfig {
    /// The id the task's own event carries.
    pub task_id: Option<String>,
    /// How long to wait before the first firing, in milliseconds.
    pub delay_ms: Option<f64>,
    /// Fire on every `delayMs`, not once.
    pub periodic: bool,
    /// `"default"` or `"healthResearch"`.
    pub task_type: Option<String>,
    /// The task needs a network connection.
    pub requires_network_connectivity: Option<bool>,
    /// Keep the schedule when the app terminates. Android only.
    pub stop_on_terminate: Option<bool>,
    /// Start the schedule at boot. Android only.
    pub start_on_boot: Option<bool>,
    /// Run the handler when the app is not running. Android only.
    pub enable_headless: Option<bool>,
    /// Schedule with AlarmManager, not JobScheduler. Android only.
    pub force_alarm_manager: Option<bool>,
    /// `"none"`, `"any"`, `"unmetered"`, `"notRoaming"` or `"cellular"`.
    pub required_network_type: Option<String>,
    /// Refuse to run when the battery is low.
    pub requires_battery_not_low: Option<bool>,
    /// Refuse to run when storage is low.
    pub requires_storage_not_low: Option<bool>,
    /// Refuse to run unless the device is charging.
    pub requires_charging: Option<bool>,
    /// Refuse to run unless the device is idle.
    pub requires_device_idle: Option<bool>,
}

/// One-off work options, from TSX. The Rust and TS APIs carry input data; this native
/// mirror has no input field in v0.1.
#[derive(JsObject, Clone, Debug, Default)]
pub struct NativeOneOffOptions {
    /// How long to wait before the work runs, in milliseconds.
    pub initial_delay_ms: Option<f64>,
    /// `"none"`, `"any"`, `"unmetered"`, `"notRoaming"` or `"cellular"`.
    pub required_network_type: Option<String>,
    /// Refuse to run when the battery is low.
    pub requires_battery_not_low: Option<bool>,
    /// Refuse to run unless the device is charging.
    pub requires_charging: Option<bool>,
    /// Refuse to run unless the device is idle.
    pub requires_device_idle: Option<bool>,
    /// Refuse to run when storage is low.
    pub requires_storage_not_low: Option<bool>,
    /// `"keep"`, `"replace"`, `"append"` or `"update"`.
    pub existing_work_policy: Option<String>,
    /// `"linear"` or `"exponential"`.
    pub backoff_policy: Option<String>,
    /// The first backoff delay, in milliseconds.
    pub backoff_policy_delay_ms: Option<f64>,
    /// A name to cancel the work by, besides its unique name.
    pub tag: Option<String>,
}

/// Periodic work options, from TSX. The Rust and TS APIs carry input data; this native
/// mirror has no input field in v0.1.
#[derive(JsObject, Clone, Debug, Default)]
pub struct NativePeriodicOptions {
    /// The period the work fires on, in milliseconds. 15 minutes by default.
    pub frequency_ms: Option<f64>,
    /// How far into a period the work may still fire, in milliseconds.
    pub flex_interval_ms: Option<f64>,
    /// How long to wait before the first firing, in milliseconds.
    pub initial_delay_ms: Option<f64>,
    /// `"none"`, `"any"`, `"unmetered"`, `"notRoaming"` or `"cellular"`.
    pub required_network_type: Option<String>,
    /// Refuse to run when the battery is low.
    pub requires_battery_not_low: Option<bool>,
    /// Refuse to run unless the device is charging.
    pub requires_charging: Option<bool>,
    /// Refuse to run unless the device is idle.
    pub requires_device_idle: Option<bool>,
    /// Refuse to run when storage is low.
    pub requires_storage_not_low: Option<bool>,
    /// `"keep"`, `"replace"` or `"update"`.
    pub existing_periodic_work_policy: Option<String>,
    /// `"linear"` or `"exponential"`.
    pub backoff_policy: Option<String>,
    /// The first backoff delay, in milliseconds.
    pub backoff_policy_delay_ms: Option<f64>,
    /// A name to cancel the work by, besides its unique name.
    pub tag: Option<String>,
}

/// What a piece of work looks like from TSX.
#[derive(JsObject, Clone, Debug)]
pub struct NativeWorkInfo {
    /// The unique name the work was registered under.
    pub unique_name: String,
    /// `"scheduled"`, `"running"`, `"succeeded"`, `"failed"` or `"cancelled"`.
    pub state: String,
    /// Whether it fires on a period.
    pub is_periodic: bool,
    /// The task the work's event names, when it has one.
    pub task_name: Option<String>,
    /// The tags the work was registered with.
    pub tags: Vec<String>,
}

/// The background scheduler and work manager in one handle. Create one and keep it; every
/// handle shares the platform's backend.
#[class]
pub struct NativeBackgroundFetch {
    fetch: BackgroundFetch,
    work: Workmanager,
}

#[class]
impl NativeBackgroundFetch {
    /// The platform's scheduler and work manager.
    pub fn new() -> NativeBackgroundFetch {
        NativeBackgroundFetch {
            fetch: BackgroundFetch::new(),
            work: Workmanager::new(),
        }
    }

    /// Whether background work runs here: `0` restricted, `1` denied, `2` available.
    pub async fn status(this: Instance<Self>) -> Result<i32, JsError> {
        let fetch = this.with(|native| native.fetch.clone());
        Ok(i32::from(fetch.status().map_err(into_js)?.as_int()))
    }

    /// Stores the fetch settings. Events are dropped until an NSH bridge delivers them.
    pub async fn configure(
        this: Instance<Self>,
        config: Option<NativeFetchConfig>,
    ) -> Result<i32, JsError> {
        let fetch = this.with(|native| native.fetch.clone());
        let config = to_fetch_config(config.unwrap_or_default())?;
        let status = fetch.configure(config, |_event| {}).map_err(into_js)?;
        Ok(i32::from(status.as_int()))
    }

    /// Arms the periodic fetch timer. Requires `configure` first.
    pub async fn start(this: Instance<Self>) -> Result<i32, JsError> {
        let fetch = this.with(|native| native.fetch.clone());
        Ok(i32::from(fetch.start().map_err(into_js)?.as_int()))
    }

    /// Stops the fetch and every timer, or just the task `taskId` names. An empty id is
    /// refused.
    pub async fn stop(this: Instance<Self>, task_id: Option<String>) -> Result<bool, JsError> {
        let fetch = this.with(|native| native.fetch.clone());
        fetch.stop(task_id.as_deref()).map_err(into_js)
    }

    /// Arms a task's own timer, once or on a period.
    pub async fn schedule_task(
        this: Instance<Self>,
        config: NativeTaskConfig,
    ) -> Result<bool, JsError> {
        let fetch = this.with(|native| native.fetch.clone());
        fetch
            .schedule_task(to_task_config(config)?)
            .map_err(into_js)
    }

    /// Tells the engine the handler is done with a task. Always succeeds.
    pub async fn finish(this: Instance<Self>, task_id: String) -> Result<(), JsError> {
        let fetch = this.with(|native| native.fetch.clone());
        fetch.finish(&task_id).map_err(into_js)
    }

    /// Registers work that fires once.
    pub async fn register_one_off_task(
        this: Instance<Self>,
        unique_name: String,
        task_name: String,
        options: Option<NativeOneOffOptions>,
    ) -> Result<(), JsError> {
        let work = this.with(|native| native.work.clone());
        work.register_one_off_task(
            &unique_name,
            &task_name,
            to_one_off_options(options.unwrap_or_default())?,
        )
        .map_err(into_js)
    }

    /// Registers work that fires on every period.
    pub async fn register_periodic_task(
        this: Instance<Self>,
        unique_name: String,
        task_name: String,
        options: Option<NativePeriodicOptions>,
    ) -> Result<(), JsError> {
        let work = this.with(|native| native.work.clone());
        work.register_periodic_task(
            &unique_name,
            &task_name,
            to_periodic_options(options.unwrap_or_default())?,
        )
        .map_err(into_js)
    }

    /// Cancels the work under `uniqueName`.
    pub async fn cancel_by_unique_name(
        this: Instance<Self>,
        unique_name: String,
    ) -> Result<(), JsError> {
        let work = this.with(|native| native.work.clone());
        work.cancel_by_unique_name(&unique_name).map_err(into_js)
    }

    /// Cancels every work registered with `tag`.
    pub async fn cancel_by_tag(this: Instance<Self>, tag: String) -> Result<(), JsError> {
        let work = this.with(|native| native.work.clone());
        work.cancel_by_tag(&tag).map_err(into_js)
    }

    /// Cancels every piece of work.
    pub async fn cancel_all(this: Instance<Self>) -> Result<(), JsError> {
        let work = this.with(|native| native.work.clone());
        work.cancel_all().map_err(into_js)
    }

    /// Whether the work under `uniqueName` is scheduled or running.
    pub async fn is_scheduled_by_unique_name(
        this: Instance<Self>,
        unique_name: String,
    ) -> Result<bool, JsError> {
        let work = this.with(|native| native.work.clone());
        work.is_scheduled_by_unique_name(&unique_name)
            .map_err(into_js)
    }

    /// The work under `uniqueName`, when it exists.
    pub async fn get_work_info(
        this: Instance<Self>,
        unique_name: String,
    ) -> Result<Option<NativeWorkInfo>, JsError> {
        let work = this.with(|native| native.work.clone());
        Ok(work
            .get_work_info(&unique_name)
            .map_err(into_js)?
            .map(native_work_info))
    }
}

impl Default for NativeBackgroundFetch {
    fn default() -> Self {
        Self::new()
    }
}

/// The error a rejection carries: its message, and the `code` apps match on.
fn into_js(error: BackgroundError) -> JsError {
    JsError::Error(error.kind().message().to_owned()).with_code(error.code())
}

fn invalid(message: &str) -> JsError {
    JsError::Error(message.to_owned()).with_code(ErrorKind::InvalidArgument.code())
}

/// `"none"` by absence; an unknown string is an invalid argument, as the references refuse
/// a strength they do not know.
fn network_type(value: Option<&str>) -> Result<NetworkType, JsError> {
    match value {
        None | Some("none") => Ok(NetworkType::None),
        Some("any") => Ok(NetworkType::Any),
        Some("unmetered") => Ok(NetworkType::Unmetered),
        Some("notRoaming") => Ok(NetworkType::NotRoaming),
        Some("cellular") => Ok(NetworkType::Cellular),
        Some(_) => Err(invalid(
            "requiredNetworkType must be \"none\", \"any\", \"unmetered\", \"notRoaming\" or \"cellular\"",
        )),
    }
}

fn task_type(value: Option<&str>) -> Result<TaskType, JsError> {
    match value {
        None | Some("default") => Ok(TaskType::Default),
        Some("healthResearch") => Ok(TaskType::HealthResearch),
        Some(_) => Err(invalid(
            "taskType must be \"default\" or \"healthResearch\"",
        )),
    }
}

fn existing_work_policy(value: Option<&str>) -> Result<Option<ExistingWorkPolicy>, JsError> {
    match value {
        None => Ok(None),
        Some("keep") => Ok(Some(ExistingWorkPolicy::Keep)),
        Some("replace") => Ok(Some(ExistingWorkPolicy::Replace)),
        Some("append") => Ok(Some(ExistingWorkPolicy::Append)),
        Some("update") => Ok(Some(ExistingWorkPolicy::Update)),
        Some(_) => Err(invalid(
            "existingWorkPolicy must be \"keep\", \"replace\", \"append\" or \"update\"",
        )),
    }
}

fn existing_periodic_work_policy(
    value: Option<&str>,
) -> Result<Option<ExistingPeriodicWorkPolicy>, JsError> {
    match value {
        None => Ok(None),
        Some("keep") => Ok(Some(ExistingPeriodicWorkPolicy::Keep)),
        Some("replace") => Ok(Some(ExistingPeriodicWorkPolicy::Replace)),
        Some("update") => Ok(Some(ExistingPeriodicWorkPolicy::Update)),
        Some(_) => Err(invalid(
            "existingPeriodicWorkPolicy must be \"keep\", \"replace\" or \"update\"",
        )),
    }
}

fn backoff_policy(value: Option<&str>) -> Result<Option<BackoffPolicy>, JsError> {
    match value {
        None => Ok(None),
        Some("linear") => Ok(Some(BackoffPolicy::Linear)),
        Some("exponential") => Ok(Some(BackoffPolicy::Exponential)),
        Some(_) => Err(invalid(
            "backoffPolicy must be \"linear\" or \"exponential\"",
        )),
    }
}

/// The constraints of work options. Every field unset stays unset, so an absent object
/// never turns into an all-off one.
#[allow(clippy::too_many_arguments)]
fn constraints(
    battery: Option<bool>,
    charging: Option<bool>,
    idle: Option<bool>,
    storage: Option<bool>,
    network: Option<&str>,
) -> Result<Option<Constraints>, JsError> {
    if battery.is_none()
        && charging.is_none()
        && idle.is_none()
        && storage.is_none()
        && network.is_none()
    {
        return Ok(None);
    }
    Ok(Some(Constraints {
        requires_battery_not_low: battery.unwrap_or(false),
        requires_charging: charging.unwrap_or(false),
        requires_device_idle: idle.unwrap_or(false),
        requires_storage_not_low: storage.unwrap_or(false),
        required_network_type: match network {
            None => None,
            Some(_) => Some(network_type(network)?),
        },
    }))
}

/// Milliseconds to a duration; a negative or non-finite number is an invalid argument.
fn duration_ms(value: Option<f64>) -> Result<Option<Duration>, JsError> {
    match value {
        None => Ok(None),
        Some(ms) if ms.is_finite() && ms >= 0.0 => Ok(Some(Duration::from_millis(ms as u64))),
        Some(_) => Err(invalid("a millisecond value cannot be negative or NaN")),
    }
}

/// Minutes to seconds; a negative or non-finite number is an invalid argument.
fn interval_minutes(minutes: Option<f64>) -> Result<Duration, JsError> {
    match minutes {
        None => Ok(FetchConfig::default().minimum_fetch_interval),
        Some(m) if m.is_finite() && m >= 0.0 => Ok(Duration::from_secs_f64(m * 60.0)),
        Some(_) => Err(invalid(
            "minimumFetchIntervalMinutes cannot be negative or NaN",
        )),
    }
}

fn to_fetch_config(native: NativeFetchConfig) -> Result<FetchConfig, JsError> {
    Ok(FetchConfig {
        minimum_fetch_interval: interval_minutes(native.minimum_fetch_interval_minutes)?,
        stop_on_terminate: native.stop_on_terminate.unwrap_or(true),
        start_on_boot: native.start_on_boot.unwrap_or(false),
        enable_headless: native.enable_headless.unwrap_or(false),
        force_alarm_manager: native.force_alarm_manager.unwrap_or(false),
        required_network_type: network_type(native.required_network_type.as_deref())?,
        requires_battery_not_low: native.requires_battery_not_low.unwrap_or(false),
        requires_storage_not_low: native.requires_storage_not_low.unwrap_or(false),
        requires_charging: native.requires_charging.unwrap_or(false),
        requires_device_idle: native.requires_device_idle.unwrap_or(false),
    })
}

fn to_task_config(native: NativeTaskConfig) -> Result<TaskConfig, JsError> {
    Ok(TaskConfig {
        task_id: native.task_id.unwrap_or_default(),
        delay: duration_ms(native.delay_ms)?.unwrap_or_default(),
        periodic: native.periodic,
        task_type: task_type(native.task_type.as_deref())?,
        requires_network_connectivity: native.requires_network_connectivity.unwrap_or(false),
        // The Android-only fields keep FetchConfig's own defaults.
        stop_on_terminate: native.stop_on_terminate.unwrap_or(true),
        start_on_boot: native.start_on_boot.unwrap_or(false),
        enable_headless: native.enable_headless.unwrap_or(false),
        force_alarm_manager: native.force_alarm_manager.unwrap_or(false),
        required_network_type: network_type(native.required_network_type.as_deref())?,
        requires_battery_not_low: native.requires_battery_not_low.unwrap_or(false),
        requires_storage_not_low: native.requires_storage_not_low.unwrap_or(false),
        requires_charging: native.requires_charging.unwrap_or(false),
        requires_device_idle: native.requires_device_idle.unwrap_or(false),
    })
}

fn to_one_off_options(native: NativeOneOffOptions) -> Result<OneOffOptions, JsError> {
    Ok(OneOffOptions {
        input: Vec::new(),
        initial_delay: duration_ms(native.initial_delay_ms)?,
        constraints: constraints(
            native.requires_battery_not_low,
            native.requires_charging,
            native.requires_device_idle,
            native.requires_storage_not_low,
            native.required_network_type.as_deref(),
        )?,
        existing_work_policy: existing_work_policy(native.existing_work_policy.as_deref())?,
        backoff_policy: backoff_policy(native.backoff_policy.as_deref())?,
        backoff_policy_delay: duration_ms(native.backoff_policy_delay_ms)?,
        tag: native.tag,
    })
}

fn to_periodic_options(native: NativePeriodicOptions) -> Result<PeriodicOptions, JsError> {
    Ok(PeriodicOptions {
        frequency: duration_ms(native.frequency_ms)?,
        flex_interval: duration_ms(native.flex_interval_ms)?,
        input: Vec::new(),
        initial_delay: duration_ms(native.initial_delay_ms)?,
        constraints: constraints(
            native.requires_battery_not_low,
            native.requires_charging,
            native.requires_device_idle,
            native.requires_storage_not_low,
            native.required_network_type.as_deref(),
        )?,
        existing_periodic_work_policy: existing_periodic_work_policy(
            native.existing_periodic_work_policy.as_deref(),
        )?,
        backoff_policy: backoff_policy(native.backoff_policy.as_deref())?,
        backoff_policy_delay: duration_ms(native.backoff_policy_delay_ms)?,
        tag: native.tag,
    })
}

fn state_name(state: WorkState) -> &'static str {
    match state {
        WorkState::Scheduled => "scheduled",
        WorkState::Running => "running",
        WorkState::Succeeded => "succeeded",
        WorkState::Failed => "failed",
        WorkState::Cancelled => "cancelled",
    }
}

fn native_work_info(info: WorkInfo) -> NativeWorkInfo {
    NativeWorkInfo {
        unique_name: info.unique_name,
        state: state_name(info.state).to_owned(),
        is_periodic: info.is_periodic,
        task_name: info.task_name,
        tags: info.tags,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unset_fetch_config_keeps_the_defaults() {
        assert_eq!(
            to_fetch_config(NativeFetchConfig::default()).unwrap(),
            FetchConfig::default()
        );
    }

    #[test]
    fn fetch_config_values_map_minutes_to_seconds() {
        let config = to_fetch_config(NativeFetchConfig {
            minimum_fetch_interval_minutes: Some(10.0),
            stop_on_terminate: Some(false),
            start_on_boot: Some(true),
            enable_headless: Some(true),
            force_alarm_manager: Some(true),
            required_network_type: Some("notRoaming".to_owned()),
            requires_battery_not_low: Some(true),
            requires_storage_not_low: Some(true),
            requires_charging: Some(true),
            requires_device_idle: Some(true),
        })
        .unwrap();
        assert_eq!(config.minimum_fetch_interval, Duration::from_secs(10 * 60));
        assert!(!config.stop_on_terminate);
        assert!(config.start_on_boot);
        assert!(config.enable_headless);
        assert!(config.force_alarm_manager);
        assert_eq!(config.required_network_type, NetworkType::NotRoaming);
        assert!(config.requires_battery_not_low);
        assert!(config.requires_storage_not_low);
        assert!(config.requires_charging);
        assert!(config.requires_device_idle);
    }

    #[test]
    fn unknown_network_task_and_policy_strings_are_refused() {
        let network = to_fetch_config(NativeFetchConfig {
            required_network_type: Some("wifi".to_owned()),
            ..NativeFetchConfig::default()
        })
        .unwrap_err();
        assert_eq!(network.code(), Some("ERR_BACKGROUND_INVALID_ARGUMENT"));

        let task = to_task_config(NativeTaskConfig {
            task_type: Some("mega".to_owned()),
            ..NativeTaskConfig::default()
        })
        .unwrap_err();
        assert_eq!(task.code(), Some("ERR_BACKGROUND_INVALID_ARGUMENT"));

        let policy = to_one_off_options(NativeOneOffOptions {
            existing_work_policy: Some("merge".to_owned()),
            ..NativeOneOffOptions::default()
        })
        .unwrap_err();
        assert_eq!(policy.code(), Some("ERR_BACKGROUND_INVALID_ARGUMENT"));

        let periodic_policy = to_periodic_options(NativePeriodicOptions {
            existing_periodic_work_policy: Some("merge".to_owned()),
            ..NativePeriodicOptions::default()
        })
        .unwrap_err();
        assert_eq!(
            periodic_policy.code(),
            Some("ERR_BACKGROUND_INVALID_ARGUMENT")
        );

        let backoff = to_one_off_options(NativeOneOffOptions {
            backoff_policy: Some("fixed".to_owned()),
            ..NativeOneOffOptions::default()
        })
        .unwrap_err();
        assert_eq!(backoff.code(), Some("ERR_BACKGROUND_INVALID_ARGUMENT"));

        let negative = to_fetch_config(NativeFetchConfig {
            minimum_fetch_interval_minutes: Some(-1.0),
            ..NativeFetchConfig::default()
        })
        .unwrap_err();
        assert_eq!(negative.code(), Some("ERR_BACKGROUND_INVALID_ARGUMENT"));
    }

    #[test]
    fn a_task_config_maps_its_milliseconds_and_kinds() {
        let config = to_task_config(NativeTaskConfig {
            task_id: Some("sync".to_owned()),
            delay_ms: Some(1500.0),
            periodic: true,
            task_type: Some("healthResearch".to_owned()),
            requires_network_connectivity: Some(true),
            stop_on_terminate: Some(false),
            required_network_type: Some("cellular".to_owned()),
            ..NativeTaskConfig::default()
        })
        .unwrap();
        assert_eq!(config.task_id, "sync");
        assert_eq!(config.delay, Duration::from_millis(1500));
        assert!(config.periodic);
        assert_eq!(config.task_type, TaskType::HealthResearch);
        assert!(config.requires_network_connectivity);
        assert!(!config.stop_on_terminate);
        assert_eq!(config.required_network_type, NetworkType::Cellular);
        // The fields TSX left out keep TaskConfig's own defaults.
        assert_eq!(
            config,
            TaskConfig {
                task_id: "sync".to_owned(),
                delay: Duration::from_millis(1500),
                periodic: true,
                task_type: TaskType::HealthResearch,
                requires_network_connectivity: true,
                stop_on_terminate: false,
                required_network_type: NetworkType::Cellular,
                ..TaskConfig::default()
            }
        );
    }

    #[test]
    fn one_off_and_periodic_options_map_their_fields() {
        let one_off = to_one_off_options(NativeOneOffOptions {
            initial_delay_ms: Some(500.0),
            required_network_type: Some("unmetered".to_owned()),
            requires_battery_not_low: Some(true),
            requires_charging: Some(true),
            requires_device_idle: Some(true),
            requires_storage_not_low: Some(true),
            existing_work_policy: Some("append".to_owned()),
            backoff_policy: Some("exponential".to_owned()),
            backoff_policy_delay_ms: Some(250.0),
            tag: Some("uploads".to_owned()),
        })
        .unwrap();
        assert_eq!(
            one_off,
            OneOffOptions {
                input: Vec::new(),
                initial_delay: Some(Duration::from_millis(500)),
                constraints: Some(Constraints {
                    requires_battery_not_low: true,
                    requires_charging: true,
                    requires_device_idle: true,
                    requires_storage_not_low: true,
                    required_network_type: Some(NetworkType::Unmetered),
                }),
                existing_work_policy: Some(ExistingWorkPolicy::Append),
                backoff_policy: Some(BackoffPolicy::Exponential),
                backoff_policy_delay: Some(Duration::from_millis(250)),
                tag: Some("uploads".to_owned()),
            }
        );

        let periodic = to_periodic_options(NativePeriodicOptions {
            frequency_ms: Some(1_800_000.0),
            flex_interval_ms: Some(300_000.0),
            existing_periodic_work_policy: Some("update".to_owned()),
            ..NativePeriodicOptions::default()
        })
        .unwrap();
        assert_eq!(
            periodic,
            PeriodicOptions {
                frequency: Some(Duration::from_millis(1_800_000)),
                flex_interval: Some(Duration::from_millis(300_000)),
                existing_periodic_work_policy: Some(ExistingPeriodicWorkPolicy::Update),
                ..PeriodicOptions::default()
            }
        );

        // A constraints object with every field unset stays unset.
        assert_eq!(
            to_one_off_options(NativeOneOffOptions::default())
                .unwrap()
                .constraints,
            None
        );

        // The reference's newest variants parse like the rest.
        let updated = to_one_off_options(NativeOneOffOptions {
            existing_work_policy: Some("update".to_owned()),
            ..NativeOneOffOptions::default()
        })
        .unwrap();
        assert_eq!(
            updated.existing_work_policy,
            Some(ExistingWorkPolicy::Update)
        );
        let replaced = to_periodic_options(NativePeriodicOptions {
            existing_periodic_work_policy: Some("replace".to_owned()),
            ..NativePeriodicOptions::default()
        })
        .unwrap();
        assert_eq!(
            replaced.existing_periodic_work_policy,
            Some(ExistingPeriodicWorkPolicy::Replace)
        );
    }

    #[test]
    fn work_info_names_the_states_tsx_matches_on() {
        let info = native_work_info(WorkInfo {
            unique_name: "sync-work".to_owned(),
            state: WorkState::Running,
            is_periodic: true,
            task_name: Some("sync".to_owned()),
            tags: vec!["uploads".to_owned()],
        });
        assert_eq!(info.unique_name, "sync-work");
        assert_eq!(info.state, "running");
        assert!(info.is_periodic);
        assert_eq!(info.task_name, Some("sync".to_owned()));
        assert_eq!(info.tags, vec!["uploads".to_owned()]);

        assert_eq!(state_name(WorkState::Scheduled), "scheduled");
        assert_eq!(state_name(WorkState::Succeeded), "succeeded");
        assert_eq!(state_name(WorkState::Failed), "failed");
        assert_eq!(state_name(WorkState::Cancelled), "cancelled");
    }
}
