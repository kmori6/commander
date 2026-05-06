CREATE TABLE job_runs (
    id UUID PRIMARY KEY,
    job_id UUID NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
    attempt INTEGER NOT NULL,
    status TEXT NOT NULL,
    started_at TIMESTAMPTZ NOT NULL,
    finished_at TIMESTAMPTZ,
    error_message TEXT,

    UNIQUE (job_id, attempt),
    CHECK (attempt > 0)
);

CREATE INDEX idx_job_runs_job_id_started_at ON job_runs(job_id, started_at DESC);
CREATE INDEX idx_job_runs_status_started_at ON job_runs(status, started_at DESC);
