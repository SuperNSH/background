//! The options the references name: statuses, network and task kinds, and the settings of a
//! fetch, a task and a piece of work.

use std::time::Duration;

/// Whether background work runs, as `background_fetch` reports it: `0` restricted, `1` denied,
/// `2` available.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Status {
    /// Background work is unavailable and the user cannot enable it again, as parental
    /// controls or a child profile make it.
    Restricted,
    /// The user switched background work off, for this app or the whole system.
    Denied,
    /// Background work runs.
    Available,
}

impl Status {
    /// The references' own number for this status.
    pub const fn as_int(self) -> u8 {
        match self {
            Status::Restricted => 0,
            Status::Denied => 1,
            Status::Available => 2,
        }
    }
}

/// The network a task or a fetch requires, as `background_fetch` numbers them: `0` none, `1`
/// any, `2` unmetered, `3` not roaming, `4` cellular.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum NetworkType {
    /// No network is required.
    #[default]
    None,
    /// Any connection counts.
    Any,
    /// A connection the device does not meter, such as Wi-Fi.
    Unmetered,
    /// Any connection that is not roaming.
    NotRoaming,
    /// A cellular connection, metered or not.
    Cellular,
}

impl NetworkType {
    /// The references' own number for this network type.
    pub const fn as_int(self) -> u8 {
        match self {
            NetworkType::None => 0,
            NetworkType::Any => 1,
            NetworkType::Unmetered => 2,
            NetworkType::NotRoaming => 3,
            NetworkType::Cellular => 4,
        }
    }
}

/// The kind of task a schedule carries. Health research gets the longer iOS budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TaskType {
    /// The plain background task budget.
    #[default]
    Default,
    /// The `HealthResearch` BGTask identifier iOS grants more time to.
    HealthResearch,
}

impl TaskType {
    /// The references' own number for this task type.
    pub const fn as_int(self) -> u8 {
        match self {
            TaskType::Default => 0,
            TaskType::HealthResearch => 1,
        }
    }
}

/// The settings of the periodic fetch. Unset fields keep these defaults.
///
/// Options a platform cannot apply are accepted and stored, as `docs/parity.md` lists them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FetchConfig {
    /// The shortest interval between two fetch events. The platform's floor applies on top.
    pub minimum_fetch_interval: Duration,
    /// Keep the schedule when the app terminates. Android only.
    pub stop_on_terminate: bool,
    /// Start the schedule at boot. Android only.
    pub start_on_boot: bool,
    /// Run the handler when the app is not running. Android only.
    pub enable_headless: bool,
    /// Schedule with AlarmManager, not JobScheduler. Android only.
    pub force_alarm_manager: bool,
    /// The network the fetch requires.
    pub required_network_type: NetworkType,
    /// Refuse to run when the battery is low.
    pub requires_battery_not_low: bool,
    /// Refuse to run when storage is low.
    pub requires_storage_not_low: bool,
    /// Refuse to run unless the device is charging.
    pub requires_charging: bool,
    /// Refuse to run unless the device is idle.
    pub requires_device_idle: bool,
}

impl Default for FetchConfig {
    /// The references' own defaults: a 15-minute fetch that survives termination.
    fn default() -> Self {
        Self {
            minimum_fetch_interval: Duration::from_secs(15 * 60),
            stop_on_terminate: true,
            start_on_boot: false,
            enable_headless: false,
            force_alarm_manager: false,
            required_network_type: NetworkType::None,
            requires_battery_not_low: false,
            requires_storage_not_low: false,
            requires_charging: false,
            requires_device_idle: false,
        }
    }
}

/// A task scheduled by name. Unset fields keep these defaults.
///
/// The nine Android-only fields carry the same defaults [`FetchConfig`] has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskConfig {
    /// The id the task's own event carries.
    pub task_id: String,
    /// How long to wait before the first firing.
    pub delay: Duration,
    /// Fire once, or keep firing every `delay`.
    pub periodic: bool,
    /// The iOS task budget the task claims.
    pub task_type: TaskType,
    /// The task needs a network connection.
    pub requires_network_connectivity: bool,
    /// Keep the schedule when the app terminates. Android only.
    pub stop_on_terminate: bool,
    /// Start the schedule at boot. Android only.
    pub start_on_boot: bool,
    /// Run the handler when the app is not running. Android only.
    pub enable_headless: bool,
    /// Schedule with AlarmManager, not JobScheduler. Android only.
    pub force_alarm_manager: bool,
    /// The network the task requires.
    pub required_network_type: NetworkType,
    /// Refuse to run when the battery is low.
    pub requires_battery_not_low: bool,
    /// Refuse to run when storage is low.
    pub requires_storage_not_low: bool,
    /// Refuse to run unless the device is charging.
    pub requires_charging: bool,
    /// Refuse to run unless the device is idle.
    pub requires_device_idle: bool,
}

impl Default for TaskConfig {
    /// The references' own defaults: FetchConfig's Android fields, no delay, one firing.
    fn default() -> Self {
        let fetch = FetchConfig::default();
        Self {
            task_id: String::new(),
            delay: Duration::ZERO,
            periodic: false,
            task_type: TaskType::Default,
            requires_network_connectivity: false,
            stop_on_terminate: fetch.stop_on_terminate,
            start_on_boot: fetch.start_on_boot,
            enable_headless: fetch.enable_headless,
            force_alarm_manager: fetch.force_alarm_manager,
            required_network_type: fetch.required_network_type,
            requires_battery_not_low: fetch.requires_battery_not_low,
            requires_storage_not_low: fetch.requires_storage_not_low,
            requires_charging: fetch.requires_charging,
            requires_device_idle: fetch.requires_device_idle,
        }
    }
}

/// The conditions work waits for. All off: the work runs as soon as it is scheduled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Constraints {
    /// The battery must not be low.
    pub requires_battery_not_low: bool,
    /// The device must be charging.
    pub requires_charging: bool,
    /// The device must be idle.
    pub requires_device_idle: bool,
    /// Storage must not be low.
    pub requires_storage_not_low: bool,
    /// The network the work requires, when it requires one.
    pub required_network_type: Option<NetworkType>,
}

/// What enqueuing work under a name that already exists does to the existing work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExistingWorkPolicy {
    /// Keep the existing work; the new registration changes nothing.
    Keep,
    /// Replace the existing work with the new one.
    Replace,
    /// Replace the existing work; in-process, the same as `Replace`, which has no work
    /// chain to append to (see `docs/parity.md`).
    Append,
    /// Update the existing work with the new specification. In-process, the same as
    /// `Replace`.
    Update,
}

/// What re-registering periodic work under a name that already exists does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExistingPeriodicWorkPolicy {
    /// Keep the existing work; the new registration changes nothing.
    Keep,
    /// Replace the existing work with the new one. The reference deprecates it for
    /// `Update`.
    Replace,
    /// Update the existing work with the new specification.
    Update,
}

/// How a failed piece of work waits before its next attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BackoffPolicy {
    /// Wait a fixed delay between attempts.
    Linear,
    /// Double the delay after every attempt.
    Exponential,
}

/// Where a piece of work is: scheduled, running, or done for good.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorkState {
    /// Waiting for its delay or its constraints.
    Scheduled,
    /// Its event has fired and it has not finished yet.
    Running,
    /// Its last run finished without an error.
    Succeeded,
    /// Its last run finished with an error.
    Failed,
    /// Cancelled; it will not run again.
    Cancelled,
}

/// What a piece of work looks like from the outside.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkInfo {
    /// The unique name the work was registered under.
    pub unique_name: String,
    /// Where the work is.
    pub state: WorkState,
    /// Whether it fires on a period.
    pub is_periodic: bool,
    /// The task the work's event names, when it has one.
    pub task_name: Option<String>,
    /// The tags the work was registered with.
    pub tags: Vec<String>,
}

/// One value of a piece of work's input data.
#[derive(Debug, Clone, PartialEq)]
pub enum InputValue {
    /// A boolean.
    Bool(bool),
    /// A whole number.
    Int(i64),
    /// A fraction.
    Double(f64),
    /// Text.
    String(String),
}

/// A piece of work's input data, as `(name, value)` pairs.
pub type Input = Vec<(String, InputValue)>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statuses_and_kinds_keep_the_references_numbers() {
        assert_eq!(Status::Restricted.as_int(), 0);
        assert_eq!(Status::Denied.as_int(), 1);
        assert_eq!(Status::Available.as_int(), 2);

        assert_eq!(NetworkType::None.as_int(), 0);
        assert_eq!(NetworkType::Any.as_int(), 1);
        assert_eq!(NetworkType::Unmetered.as_int(), 2);
        assert_eq!(NetworkType::NotRoaming.as_int(), 3);
        assert_eq!(NetworkType::Cellular.as_int(), 4);

        assert_eq!(TaskType::Default.as_int(), 0);
        assert_eq!(TaskType::HealthResearch.as_int(), 1);
    }

    #[test]
    fn a_fetch_config_defaults_to_a_quarter_hour_fetch() {
        let config = FetchConfig::default();
        assert_eq!(config.minimum_fetch_interval, Duration::from_secs(15 * 60));
        assert!(config.stop_on_terminate);
        assert!(!config.start_on_boot);
        assert!(!config.enable_headless);
        assert!(!config.force_alarm_manager);
        assert_eq!(config.required_network_type, NetworkType::None);
        assert!(!config.requires_battery_not_low);
        assert!(!config.requires_storage_not_low);
        assert!(!config.requires_charging);
        assert!(!config.requires_device_idle);
    }

    #[test]
    fn a_task_config_and_constraints_default_to_no_conditions() {
        let config = TaskConfig::default();
        assert_eq!(config.task_id, "");
        assert_eq!(config.delay, Duration::ZERO);
        assert!(!config.periodic);
        assert_eq!(config.task_type, TaskType::Default);
        assert!(!config.requires_network_connectivity);
        // The nine Android-only fields carry FetchConfig's defaults.
        assert!(config.stop_on_terminate);
        assert!(!config.start_on_boot);
        assert!(!config.enable_headless);
        assert!(!config.force_alarm_manager);
        assert_eq!(config.required_network_type, NetworkType::None);
        assert!(!config.requires_battery_not_low);
        assert!(!config.requires_storage_not_low);
        assert!(!config.requires_charging);
        assert!(!config.requires_device_idle);

        let constraints = Constraints::default();
        assert!(!constraints.requires_battery_not_low);
        assert!(!constraints.requires_charging);
        assert!(!constraints.requires_device_idle);
        assert!(!constraints.requires_storage_not_low);
        assert_eq!(constraints.required_network_type, None);
    }
}
