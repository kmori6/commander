use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use cron::Schedule as CronSchedule;
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

        let schedule = parser_expression
            .parse::<CronSchedule>()
            .map_err(|err| format!("invalid cron expression: {err}"))?;

        Ok(Self {
            source: source.to_string(),
            schedule,
        })
    }

    pub fn as_str(&self) -> &str {
        &self.source
    }

    pub fn due_time(
        &self,
        timezone: ScheduleTimezone,
        now: DateTime<Utc>,
        window: chrono::Duration,
    ) -> Option<DateTime<Utc>> {
        let now_local = now.with_timezone(&timezone.timezone);
        let from_local = now_local - window;

        self.schedule
            .after(&from_local)
            .next()
            .filter(|scheduled_at| *scheduled_at <= now_local)
            .map(|scheduled_at| scheduled_at.with_timezone(&Utc))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScheduleTimezone {
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

        Ok(Self { timezone })
    }

    pub fn as_str(&self) -> &str {
        self.timezone.name()
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
    pub fn due_time(&self, now: DateTime<Utc>, window: chrono::Duration) -> Option<DateTime<Utc>> {
        if !self.enabled {
            return None;
        }

        self.cron.due_time(self.timezone, now, window)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn test_schedule(cron: &str, timezone: &str) -> Schedule {
        let now = Utc.with_ymd_and_hms(2026, 5, 13, 0, 0, 0).unwrap();

        Schedule {
            id: Uuid::nil(),
            title: "test schedule".to_string(),
            request: "run test task".to_string(),
            cron: CronExpression::parse(cron).unwrap(),
            timezone: ScheduleTimezone::parse(timezone).unwrap(),
            enabled: true,
            created_at: now,
            updated_at: now,
        }
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
    fn due_time_returns_none_when_disabled() {
        let mut schedule = test_schedule("0 9 * * *", "Asia/Tokyo");
        schedule.enabled = false;
        let now = Utc.with_ymd_and_hms(2026, 5, 13, 0, 0, 10).unwrap();

        let due_time = schedule.due_time(now, chrono::Duration::seconds(60));

        assert_eq!(None, due_time);
    }

    #[test]
    fn due_time_returns_none_before_scheduled_time() {
        let schedule = test_schedule("0 9 * * *", "Asia/Tokyo");
        let now = Utc.with_ymd_and_hms(2026, 5, 12, 23, 59, 50).unwrap();

        let due_time = schedule.due_time(now, chrono::Duration::seconds(60));

        assert_eq!(None, due_time);
    }
}
