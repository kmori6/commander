use crate::domain::model::schedule::{CronExpression, ScheduleTimezone};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct WatchConfig {
    pub enabled: bool,
    pub schedules: Vec<WatchSchedule>,
}

#[derive(Debug, Clone)]
pub struct WatchSchedule {
    pub cron: CronExpression,
    pub timezone: ScheduleTimezone,
}

impl WatchSchedule {
    pub fn restore(cron: String, timezone: String) -> Result<Self, String> {
        Ok(Self {
            cron: CronExpression::parse(&cron)?,
            timezone: ScheduleTimezone::parse(&timezone)?,
        })
    }

    pub fn due_time(&self, now: DateTime<Utc>, window: chrono::Duration) -> Option<DateTime<Utc>> {
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

impl WatchConfig {
    pub fn due_time(&self, now: DateTime<Utc>, window: chrono::Duration) -> Option<DateTime<Utc>> {
        if !self.enabled {
            return None;
        }

        self.schedules
            .iter()
            .filter_map(|schedule| schedule.due_time(now, window))
            .min()
    }
}
