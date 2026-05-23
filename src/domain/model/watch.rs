use crate::domain::model::schedule::{CronExpression, ScheduleTimezone};
use chrono::{DateTime, Utc};

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
        self.cron.due_time(self.timezone, now, window)
    }
}

#[derive(Debug, Clone)]
pub struct Watch {
    pub enabled: bool,
    pub schedules: Vec<WatchSchedule>,
}

impl Watch {
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
