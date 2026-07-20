use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{
    DateTime, Datelike, Duration as ChronoDuration, Local, NaiveTime, TimeZone, Timelike,
};
use freehand_contracts::{AgentId, SessionId, TraceId, TurnId};
use freehand_reason::TurnRecord;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use thiserror::Error;

use crate::{now_unix_seconds, sanitize_identifier};

static GENERATED_TIMER_ID_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum TimerRepeatRule {
    Interval {
        interval_seconds: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_runs: Option<u32>,
    },
    Daily {
        #[serde(alias = "time_of_day_seconds_utc")]
        time_of_day_seconds_local: u32,
        #[serde(default)]
        skip_weekends: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_runs: Option<u32>,
    },
    Weekly {
        #[serde(alias = "time_of_day_seconds_utc")]
        time_of_day_seconds_local: u32,
        weekdays: Vec<u8>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_runs: Option<u32>,
    },
    Cron {
        expression: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_runs: Option<u32>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TimerSchedule {
    pub schema_version: u32,
    pub timer_id: String,
    pub agent_id: AgentId,
    pub status: String,
    pub reason: String,
    pub prompt: String,
    pub next_due_at: u64,
    pub created_at: u64,
    pub updated_at: u64,
    pub fired_count: u32,
    pub max_runs: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeat: Option<TimerRepeatRule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_session_id: Option<SessionId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_turn_id: Option<TurnId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_trace_id: Option<TraceId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TimerLedgerEvent {
    pub(crate) schema_version: u32,
    pub(crate) event_id: String,
    pub(crate) timer_id: String,
    pub(crate) agent_id: AgentId,
    pub(crate) event_type: String,
    pub(crate) occurred_at: u64,
    pub(crate) payload: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DueTimerSchedule {
    pub schedule: TimerSchedule,
    pub fired_at: u64,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum TimerStoreError {
    #[error("timer field `{0}` is required")]
    MissingField(&'static str),
    #[error("timer `{0}` not found")]
    NotFound(String),
    #[error("timer persistence failed: {0}")]
    Persistence(String),
    #[error("timer repeat rule invalid: {0}")]
    InvalidRepeat(String),
}

pub(crate) struct TimerStore {
    runtime_home: PathBuf,
    agent_id: AgentId,
}

impl TimerStore {
    pub(crate) fn new(runtime_home: &Path, agent_id: &AgentId) -> Self {
        Self {
            runtime_home: runtime_home.to_path_buf(),
            agent_id: agent_id.clone(),
        }
    }

    pub(crate) fn schedule_from_args(
        &self,
        args: &Map<String, Value>,
        turn: &TurnRecord,
    ) -> Result<TimerSchedule, TimerStoreError> {
        let now = now_unix_seconds();
        let reason = required_timer_string(args, "reason")?.to_owned();
        let prompt = required_timer_string(args, "prompt")?.to_owned();
        let mode = required_timer_string(args, "mode")?;
        let repeat = parse_timer_repeat(args)?;
        let next_due_at = match mode {
            "relative" => {
                let delay = required_timer_u64(args, "delay_seconds")?;
                now.saturating_add(delay)
            }
            "absolute" => required_timer_u64(args, "run_at_unix_seconds")?,
            "recurring" => {
                let repeat = repeat
                    .as_ref()
                    .ok_or(TimerStoreError::MissingField("repeat"))?;
                next_due_after(now, repeat).ok_or_else(|| {
                    TimerStoreError::InvalidRepeat("cannot compute next recurring fire".to_owned())
                })?
            }
            other => {
                return Err(TimerStoreError::InvalidRepeat(format!(
                    "unsupported mode `{other}`"
                )));
            }
        };
        let max_runs = optional_timer_u32(args, "max_runs")
            .or_else(|| repeat.as_ref().and_then(repeat_max_runs))
            .unwrap_or(1);
        if max_runs == 0 {
            return Err(TimerStoreError::MissingField("max_runs"));
        }
        Ok(TimerSchedule {
            schema_version: 1,
            timer_id: optional_timer_string(args, "timer_id")
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| generated_timer_id(turn)),
            agent_id: self.agent_id.clone(),
            status: "active".to_owned(),
            reason,
            prompt,
            next_due_at,
            created_at: now,
            updated_at: now,
            fired_count: 0,
            max_runs,
            repeat,
            source_session_id: Some(turn.request.session_id.clone()),
            source_turn_id: Some(turn.request.turn_id.clone()),
            source_trace_id: Some(turn.request.trace_id.clone()),
        })
    }

    pub(crate) fn upsert_schedule(
        &self,
        schedule: TimerSchedule,
    ) -> Result<TimerSchedule, TimerStoreError> {
        let mut schedules = self.load_schedules()?;
        schedules.retain(|existing| existing.timer_id != schedule.timer_id);
        schedules.push(schedule.clone());
        self.write_schedules(&schedules)?;
        self.append_event(
            &schedule,
            "TimerScheduled",
            json!({
                "reason": schedule.reason,
                "next_due_at": schedule.next_due_at,
                "max_runs": schedule.max_runs,
                "repeat": schedule.repeat,
            }),
        )?;
        Ok(schedule)
    }

    pub(crate) fn cancel(&self, timer_id: &str) -> Result<TimerSchedule, TimerStoreError> {
        let mut schedules = self.load_schedules()?;
        let mut cancelled = None;
        for schedule in &mut schedules {
            if schedule.timer_id == timer_id {
                schedule.status = "cancelled".to_owned();
                schedule.updated_at = now_unix_seconds();
                cancelled = Some(schedule.clone());
                break;
            }
        }
        let schedule = cancelled.ok_or_else(|| TimerStoreError::NotFound(timer_id.to_owned()))?;
        self.write_schedules(&schedules)?;
        self.append_event(&schedule, "TimerCancelled", json!({}))?;
        Ok(schedule)
    }

    pub(crate) fn active_schedules(&self) -> Result<Vec<TimerSchedule>, TimerStoreError> {
        Ok(self
            .load_schedules()?
            .into_iter()
            .filter(|schedule| schedule.status == "active")
            .collect())
    }

    fn claim_due(&self, now: u64) -> Result<Option<DueTimerSchedule>, TimerStoreError> {
        let mut schedules = self.load_schedules()?;
        let Some(index) = schedules
            .iter()
            .position(|schedule| schedule.status == "active" && schedule.next_due_at <= now)
        else {
            return Ok(None);
        };
        let mut schedule = schedules[index].clone();
        schedule.status = "running".to_owned();
        schedule.updated_at = now;
        schedules[index] = schedule.clone();
        self.write_schedules(&schedules)?;
        self.append_event(&schedule, "TimerFired", json!({"fired_at": now}))?;
        Ok(Some(DueTimerSchedule {
            schedule,
            fired_at: now,
        }))
    }

    fn complete_due(&self, due: &DueTimerSchedule) -> Result<TimerSchedule, TimerStoreError> {
        let mut schedules = self.load_schedules()?;
        let Some(index) = schedules
            .iter()
            .position(|schedule| schedule.timer_id == due.schedule.timer_id)
        else {
            return Err(TimerStoreError::NotFound(due.schedule.timer_id.clone()));
        };
        let mut schedule = schedules[index].clone();
        schedule.fired_count = schedule.fired_count.saturating_add(1);
        schedule.updated_at = now_unix_seconds();
        if schedule.fired_count >= schedule.max_runs {
            schedule.status = "completed".to_owned();
        } else if let Some(repeat) = schedule.repeat.as_ref() {
            schedule.status = "active".to_owned();
            schedule.next_due_at = next_due_after(due.fired_at.saturating_add(1), repeat)
                .ok_or_else(|| {
                    TimerStoreError::InvalidRepeat("cannot compute next recurring fire".to_owned())
                })?;
        } else {
            schedule.status = "completed".to_owned();
        }
        schedules[index] = schedule.clone();
        self.write_schedules(&schedules)?;
        self.append_event(
            &schedule,
            "TimerCompleted",
            json!({
                "fired_count": schedule.fired_count,
                "status": schedule.status,
                "next_due_at": schedule.next_due_at,
            }),
        )?;
        Ok(schedule)
    }

    fn fail_due(
        &self,
        due: &DueTimerSchedule,
        error: &str,
    ) -> Result<TimerSchedule, TimerStoreError> {
        let mut schedules = self.load_schedules()?;
        let Some(index) = schedules
            .iter()
            .position(|schedule| schedule.timer_id == due.schedule.timer_id)
        else {
            return Err(TimerStoreError::NotFound(due.schedule.timer_id.clone()));
        };
        let mut schedule = schedules[index].clone();
        schedule.status = "active".to_owned();
        schedule.updated_at = now_unix_seconds();
        schedules[index] = schedule.clone();
        self.write_schedules(&schedules)?;
        self.append_event(
            &schedule,
            "TimerFailed",
            json!({
                "fired_at": due.fired_at,
                "error": error,
                "next_due_at": schedule.next_due_at,
            }),
        )?;
        Ok(schedule)
    }

    pub(crate) fn load_schedules(&self) -> Result<Vec<TimerSchedule>, TimerStoreError> {
        let path = self.state_path();
        if !path.is_file() {
            return Ok(Vec::new());
        }
        let raw = fs::read_to_string(&path)
            .map_err(|error| TimerStoreError::Persistence(error.to_string()))?;
        serde_json::from_str(&raw).map_err(|error| TimerStoreError::Persistence(error.to_string()))
    }

    fn write_schedules(&self, schedules: &[TimerSchedule]) -> Result<(), TimerStoreError> {
        let path = self.state_path();
        let parent = path.parent().ok_or_else(|| {
            TimerStoreError::Persistence("timer state path has no parent".to_owned())
        })?;
        fs::create_dir_all(parent)
            .map_err(|error| TimerStoreError::Persistence(error.to_string()))?;
        let temp = path.with_extension("tmp");
        let raw = serde_json::to_string_pretty(schedules)
            .map_err(|error| TimerStoreError::Persistence(error.to_string()))?;
        fs::write(&temp, raw).map_err(|error| TimerStoreError::Persistence(error.to_string()))?;
        fs::rename(&temp, &path).map_err(|error| TimerStoreError::Persistence(error.to_string()))
    }

    fn append_event(
        &self,
        schedule: &TimerSchedule,
        event_type: &str,
        payload: Value,
    ) -> Result<(), TimerStoreError> {
        let path = self.ledger_path();
        let parent = path.parent().ok_or_else(|| {
            TimerStoreError::Persistence("timer ledger path has no parent".to_owned())
        })?;
        fs::create_dir_all(parent)
            .map_err(|error| TimerStoreError::Persistence(error.to_string()))?;
        let event = TimerLedgerEvent {
            schema_version: 1,
            event_id: format!(
                "timer-event-{}-{}",
                sanitize_identifier(&schedule.timer_id),
                now_unix_seconds()
            ),
            timer_id: schedule.timer_id.clone(),
            agent_id: self.agent_id.clone(),
            event_type: event_type.to_owned(),
            occurred_at: now_unix_seconds(),
            payload,
        };
        let line = serde_json::to_string(&event)
            .map_err(|error| TimerStoreError::Persistence(error.to_string()))?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|error| TimerStoreError::Persistence(error.to_string()))?;
        writeln!(file, "{line}").map_err(|error| TimerStoreError::Persistence(error.to_string()))
    }

    #[cfg(test)]
    pub(crate) fn load_events(&self) -> Result<Vec<TimerLedgerEvent>, TimerStoreError> {
        use std::io::{BufRead, BufReader};

        let path = self.ledger_path();
        if !path.is_file() {
            return Ok(Vec::new());
        }
        let file = fs::File::open(path)
            .map_err(|error| TimerStoreError::Persistence(error.to_string()))?;
        let reader = BufReader::new(file);
        let mut events = Vec::new();
        for line in reader.lines() {
            let line = line.map_err(|error| TimerStoreError::Persistence(error.to_string()))?;
            if !line.trim().is_empty() {
                events.push(
                    serde_json::from_str(&line)
                        .map_err(|error| TimerStoreError::Persistence(error.to_string()))?,
                );
            }
        }
        Ok(events)
    }

    fn state_path(&self) -> PathBuf {
        self.runtime_home
            .join("state")
            .join("timers")
            .join(format!("{}.json", self.agent_id.as_str()))
    }

    fn ledger_path(&self) -> PathBuf {
        self.runtime_home
            .join("ledgers")
            .join("timers")
            .join(format!("{}.jsonl", self.agent_id.as_str()))
    }
}

pub(crate) fn claim_due_timer_schedule(
    runtime_home: &Path,
    agent_id: &AgentId,
    now: u64,
) -> Result<Option<DueTimerSchedule>, TimerStoreError> {
    TimerStore::new(runtime_home, agent_id).claim_due(now)
}

pub(crate) fn complete_due_timer_schedule(
    runtime_home: &Path,
    agent_id: &AgentId,
    due: &DueTimerSchedule,
) -> Result<TimerSchedule, TimerStoreError> {
    TimerStore::new(runtime_home, agent_id).complete_due(due)
}

pub(crate) fn fail_due_timer_schedule(
    runtime_home: &Path,
    agent_id: &AgentId,
    due: &DueTimerSchedule,
    error: &str,
) -> Result<TimerSchedule, TimerStoreError> {
    TimerStore::new(runtime_home, agent_id).fail_due(due, error)
}

fn generated_timer_id(turn: &TurnRecord) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let sequence = GENERATED_TIMER_ID_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!(
        "timer-{}-{}-{}-{}-{}",
        sanitize_identifier(turn.request.agent_id.as_str()),
        sanitize_identifier(turn.request.session_id.as_str()),
        sanitize_identifier(turn.request.turn_id.as_str()),
        nanos,
        sequence
    )
}

fn required_timer_string<'a>(
    object: &'a Map<String, Value>,
    field: &'static str,
) -> Result<&'a str, TimerStoreError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(TimerStoreError::MissingField(field))
}

fn optional_timer_string<'a>(object: &'a Map<String, Value>, field: &str) -> Option<&'a str> {
    object
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn required_timer_u64(
    object: &Map<String, Value>,
    field: &'static str,
) -> Result<u64, TimerStoreError> {
    object
        .get(field)
        .and_then(Value::as_u64)
        .filter(|value| *value > 0 || field == "run_at_unix_seconds")
        .ok_or(TimerStoreError::MissingField(field))
}

fn optional_timer_u32(object: &Map<String, Value>, field: &str) -> Option<u32> {
    object
        .get(field)
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
}

fn parse_timer_repeat(
    object: &Map<String, Value>,
) -> Result<Option<TimerRepeatRule>, TimerStoreError> {
    let Some(value) = object.get("repeat") else {
        return Ok(None);
    };
    let repeat = value
        .as_object()
        .ok_or_else(|| TimerStoreError::InvalidRepeat("repeat must be an object".to_owned()))?;
    let kind = required_timer_string(repeat, "kind")?;
    let max_runs = optional_timer_u32(repeat, "max_runs");
    match kind {
        "interval" => Ok(Some(TimerRepeatRule::Interval {
            interval_seconds: required_timer_u64(repeat, "interval_seconds")?,
            max_runs,
        })),
        "daily" => Ok(Some(TimerRepeatRule::Daily {
            time_of_day_seconds_local: timer_time_of_day(repeat)?,
            skip_weekends: repeat
                .get("skip_weekends")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            max_runs,
        })),
        "weekly" => {
            let weekdays = repeat
                .get("weekdays")
                .and_then(Value::as_array)
                .ok_or(TimerStoreError::MissingField("weekdays"))?
                .iter()
                .map(|value| {
                    value
                        .as_u64()
                        .and_then(|day| u8::try_from(day).ok())
                        .filter(|day| *day <= 6)
                        .ok_or_else(|| {
                            TimerStoreError::InvalidRepeat(
                                "weekdays must be integers 0..6".to_owned(),
                            )
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            if weekdays.is_empty() {
                return Err(TimerStoreError::MissingField("weekdays"));
            }
            Ok(Some(TimerRepeatRule::Weekly {
                time_of_day_seconds_local: timer_time_of_day(repeat)?,
                weekdays,
                max_runs,
            }))
        }
        "cron" => {
            let expression = optional_timer_string(repeat, "expression")
                .or_else(|| optional_timer_string(repeat, "cron_expression"))
                .ok_or(TimerStoreError::MissingField("expression"))?
                .to_owned();
            parse_cron_expression(&expression)?;
            Ok(Some(TimerRepeatRule::Cron {
                expression,
                max_runs,
            }))
        }
        other => Err(TimerStoreError::InvalidRepeat(format!(
            "unsupported repeat kind `{other}`"
        ))),
    }
}

fn timer_time_of_day(object: &Map<String, Value>) -> Result<u32, TimerStoreError> {
    let value = object
        .get("time_of_day_seconds_local")
        .or_else(|| object.get("time_of_day_seconds_utc"))
        .and_then(Value::as_u64)
        .filter(|value| *value < 86_400)
        .ok_or(TimerStoreError::MissingField("time_of_day_seconds_local"))?;
    u32::try_from(value).map_err(|_| {
        TimerStoreError::InvalidRepeat("time_of_day_seconds_local too large".to_owned())
    })
}

fn repeat_max_runs(rule: &TimerRepeatRule) -> Option<u32> {
    match rule {
        TimerRepeatRule::Interval { max_runs, .. }
        | TimerRepeatRule::Daily { max_runs, .. }
        | TimerRepeatRule::Weekly { max_runs, .. }
        | TimerRepeatRule::Cron { max_runs, .. } => *max_runs,
    }
}

fn next_due_after(after: u64, rule: &TimerRepeatRule) -> Option<u64> {
    match rule {
        TimerRepeatRule::Interval {
            interval_seconds, ..
        } => Some(after.saturating_add(*interval_seconds)),
        TimerRepeatRule::Daily {
            time_of_day_seconds_local,
            skip_weekends,
            ..
        } => next_daily_due(after, *time_of_day_seconds_local, *skip_weekends),
        TimerRepeatRule::Weekly {
            time_of_day_seconds_local,
            weekdays,
            ..
        } => next_weekly_due(after, *time_of_day_seconds_local, weekdays),
        TimerRepeatRule::Cron { expression, .. } => next_cron_due(after, expression),
    }
}

pub(crate) fn next_daily_due(after: u64, time_of_day: u32, skip_weekends: bool) -> Option<u64> {
    let after_dt = local_datetime(after)?;
    let local_time = NaiveTime::from_num_seconds_from_midnight_opt(time_of_day, 0)?;
    for day_offset in 0..14_i64 {
        let date = after_dt.date_naive() + ChronoDuration::days(day_offset);
        let candidate = local_timestamp(date.and_time(local_time))?;
        if candidate <= after {
            continue;
        }
        let weekday = local_weekday(candidate)?;
        if skip_weekends && (weekday == 0 || weekday == 6) {
            continue;
        }
        return Some(candidate);
    }
    None
}

pub(crate) fn next_weekly_due(after: u64, time_of_day: u32, weekdays: &[u8]) -> Option<u64> {
    let after_dt = local_datetime(after)?;
    let local_time = NaiveTime::from_num_seconds_from_midnight_opt(time_of_day, 0)?;
    for day_offset in 0..14_i64 {
        let date = after_dt.date_naive() + ChronoDuration::days(day_offset);
        let candidate = local_timestamp(date.and_time(local_time))?;
        if candidate <= after {
            continue;
        }
        if weekdays.contains(&local_weekday(candidate)?) {
            return Some(candidate);
        }
    }
    None
}

fn next_cron_due(after: u64, expression: &str) -> Option<u64> {
    let cron = parse_cron_expression(expression).ok()?;
    let mut cursor = after.saturating_add(60 - (after % 60));
    for _ in 0..527_040_u32 {
        let dt = local_datetime(cursor)?;
        if cron.matches(&dt) {
            return Some(cursor);
        }
        cursor = cursor.saturating_add(60);
    }
    None
}

pub(crate) fn local_datetime(timestamp: u64) -> Option<DateTime<Local>> {
    let timestamp = i64::try_from(timestamp).ok()?;
    Local.timestamp_opt(timestamp, 0).earliest()
}

fn local_timestamp(local: chrono::NaiveDateTime) -> Option<u64> {
    let timestamp = Local.from_local_datetime(&local).earliest()?.timestamp();
    u64::try_from(timestamp).ok()
}

fn local_weekday(timestamp: u64) -> Option<u8> {
    Some(local_datetime(timestamp)?.weekday().num_days_from_sunday() as u8)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedCronExpression {
    pub(crate) minutes: Vec<u32>,
    pub(crate) hours: Vec<u32>,
    pub(crate) days_of_month: Vec<u32>,
    pub(crate) months: Vec<u32>,
    pub(crate) weekdays: Vec<u32>,
}

impl ParsedCronExpression {
    fn matches(&self, dt: &DateTime<Local>) -> bool {
        self.minutes.contains(&dt.minute())
            && self.hours.contains(&dt.hour())
            && self.days_of_month.contains(&dt.day())
            && self.months.contains(&dt.month())
            && self.weekdays.contains(&dt.weekday().num_days_from_sunday())
    }
}

pub(crate) fn parse_cron_expression(
    expression: &str,
) -> Result<ParsedCronExpression, TimerStoreError> {
    let fields = expression.split_whitespace().collect::<Vec<_>>();
    if fields.len() != 5 {
        return Err(TimerStoreError::InvalidRepeat(
            "cron expression must have 5 fields: minute hour day-of-month month weekday".to_owned(),
        ));
    }
    Ok(ParsedCronExpression {
        minutes: parse_cron_field(fields[0], 0, 59, "minute")?,
        hours: parse_cron_field(fields[1], 0, 23, "hour")?,
        days_of_month: parse_cron_field(fields[2], 1, 31, "day-of-month")?,
        months: parse_cron_field(fields[3], 1, 12, "month")?,
        weekdays: parse_cron_field(fields[4], 0, 6, "weekday")?,
    })
}

fn parse_cron_field(
    field: &str,
    min: u32,
    max: u32,
    name: &str,
) -> Result<Vec<u32>, TimerStoreError> {
    if field.trim().is_empty() {
        return Err(TimerStoreError::InvalidRepeat(format!(
            "cron {name} field is empty"
        )));
    }
    let mut values = Vec::new();
    for part in field.split(',') {
        let (range_part, step) = match part.split_once('/') {
            Some((range_part, step_part)) => {
                let step = step_part.parse::<u32>().map_err(|_| {
                    TimerStoreError::InvalidRepeat(format!("cron {name} step must be an integer"))
                })?;
                if step == 0 {
                    return Err(TimerStoreError::InvalidRepeat(format!(
                        "cron {name} step must be greater than zero"
                    )));
                }
                (range_part, step)
            }
            None => (part, 1),
        };
        let (start, end) = if range_part == "*" {
            (min, max)
        } else if let Some((start, end)) = range_part.split_once('-') {
            (
                parse_cron_number(start, min, max, name)?,
                parse_cron_number(end, min, max, name)?,
            )
        } else {
            let value = parse_cron_number(range_part, min, max, name)?;
            (value, value)
        };
        if start > end {
            return Err(TimerStoreError::InvalidRepeat(format!(
                "cron {name} range start must be <= end"
            )));
        }
        let mut value = start;
        while value <= end {
            if !values.contains(&value) {
                values.push(value);
            }
            value = value.saturating_add(step);
            if value == u32::MAX {
                break;
            }
        }
    }
    if values.is_empty() {
        return Err(TimerStoreError::InvalidRepeat(format!(
            "cron {name} field produced no values"
        )));
    }
    values.sort_unstable();
    Ok(values)
}

fn parse_cron_number(raw: &str, min: u32, max: u32, name: &str) -> Result<u32, TimerStoreError> {
    let value = raw.parse::<u32>().map_err(|_| {
        TimerStoreError::InvalidRepeat(format!("cron {name} value must be an integer"))
    })?;
    if value < min || value > max {
        return Err(TimerStoreError::InvalidRepeat(format!(
            "cron {name} value {value} outside {min}..{max}"
        )));
    }
    Ok(value)
}
