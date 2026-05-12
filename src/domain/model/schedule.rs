use chrono::{DateTime, Utc};
use cron::Schedule as CronSchedule;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CronExpression(String);

impl CronExpression {
    pub fn parse(value: &str) -> Result<Self, String> {
        let value = value.trim();

        if value.is_empty() {
            return Err("cron must not be empty".to_string());
        }

        let parser_expression = if value.starts_with('@') {
            value.to_string()
        } else {
            match value.split_whitespace().count() {
                5 => format!("0 {value}"),
                6 | 7 => value.to_string(),
                _ => {
                    return Err(
                        "cron must have 5, 6, or 7 fields, or use a supported @ shorthand"
                            .to_string(),
                    );
                }
            }
        };

        CronSchedule::from_str(&parser_expression)
            .map_err(|err| format!("invalid cron expression: {err}"))?;

        Ok(Self(value.to_string()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleTimezone(String);

impl ScheduleTimezone {
    pub fn parse(value: &str) -> Result<Self, String> {
        let value = value.trim();

        if value.is_empty() {
            return Err("timezone must not be empty".to_string());
        }

        value
            .parse::<chrono_tz::Tz>()
            .map_err(|err| format!("invalid timezone: {err}"))?;

        Ok(Self(value.to_string()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Schedule {
    pub id: Uuid,
    pub title: String,
    pub request: String,
    pub cron: String,
    pub timezone: String,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleRun {
    pub id: Uuid,
    pub schedule_id: Uuid,
    pub task_id: Uuid,
    pub scheduled_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}
