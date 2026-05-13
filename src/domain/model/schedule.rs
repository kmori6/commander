use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use cron::Schedule as CronSchedule;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CronExpression {
    source: String,
    schedule: CronSchedule,
}

impl CronExpression {
    pub fn parse(value: &str) -> Result<Self, String> {
        let source = value.trim();

        if source.is_empty() {
            return Err("cron must not be empty".to_string());
        }

        let parser_expression = if source.starts_with('@') {
            source.to_string()
        } else {
            match source.split_whitespace().count() {
                5 => format!("0 {source}"),
                6 | 7 => source.to_string(),
                _ => {
                    return Err(
                        "cron must have 5, 6, or 7 fields, or use a supported @ shorthand"
                            .to_string(),
                    );
                }
            }
        };

        let schedule = CronSchedule::from_str(&parser_expression)
            .map_err(|err| format!("invalid cron expression: {err}"))?;

        Ok(Self {
            source: source.to_string(),
            schedule,
        })
    }

    pub fn as_str(&self) -> &str {
        &self.source
    }

    pub fn schedule(&self) -> &CronSchedule {
        &self.schedule
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScheduleTimezone {
    source: &'static str,
    timezone: Tz,
}

impl ScheduleTimezone {
    pub fn parse(value: &str) -> Result<Self, String> {
        let source = value.trim();

        if source.is_empty() {
            return Err("timezone must not be empty".to_string());
        }

        let timezone = source
            .parse::<Tz>()
            .map_err(|err| format!("invalid timezone: {err}"))?;

        Ok(Self {
            source: timezone.name(),
            timezone,
        })
    }

    pub fn as_str(&self) -> &str {
        self.source
    }

    pub fn as_tz(&self) -> Tz {
        self.timezone
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schedule {
    pub id: Uuid,
    pub title: String,
    pub request: String,
    pub cron: CronExpression,
    pub timezone: ScheduleTimezone,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Schedule {
    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        id: Uuid,
        title: String,
        request: String,
        cron: String,
        timezone: String,
        enabled: bool,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Result<Self, String> {
        Ok(Self {
            id,
            title,
            request,
            cron: CronExpression::parse(&cron)?,
            timezone: ScheduleTimezone::parse(&timezone)?,
            enabled,
            created_at,
            updated_at,
        })
    }

    pub fn due_time(&self, now: DateTime<Utc>, window: chrono::Duration) -> Option<DateTime<Utc>> {
        if !self.enabled {
            return None;
        }

        let now_local = now.with_timezone(&self.timezone.as_tz());
        let from_local = now_local - window;

        self.cron
            .schedule()
            .after(&from_local)
            .next()
            .filter(|scheduled_at| *scheduled_at <= now_local)
            .map(|scheduled_at| scheduled_at.with_timezone(&Utc))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn test_schedule(cron: &str, timezone: &str) -> Schedule {
        let now = Utc.with_ymd_and_hms(2026, 5, 13, 0, 0, 0).unwrap();

        Schedule::restore(
            Uuid::nil(),
            "test schedule".to_string(),
            "run test task".to_string(),
            cron.to_string(),
            timezone.to_string(),
            true,
            now,
            now,
        )
        .unwrap()
    }

    #[test]
    fn due_time_returns_scheduled_time_in_utc() {
        let schedule = test_schedule("0 9 * * *", "Asia/Tokyo");
        let now = Utc.with_ymd_and_hms(2026, 5, 13, 0, 0, 10).unwrap();

        let due_time = schedule.due_time(now, chrono::Duration::seconds(60));

        assert_eq!(
            Some(Utc.with_ymd_and_hms(2026, 5, 13, 0, 0, 0).unwrap()),
            due_time
        );
    }

    #[test]
    fn due_time_returns_none_before_scheduled_time() {
        let schedule = test_schedule("0 9 * * *", "Asia/Tokyo");
        let now = Utc.with_ymd_and_hms(2026, 5, 12, 23, 59, 50).unwrap();

        let due_time = schedule.due_time(now, chrono::Duration::seconds(60));

        assert_eq!(None, due_time);
    }
}
