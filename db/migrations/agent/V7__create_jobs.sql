CREATE TABLE jobs (
    id UUID PRIMARY KEY,
    kind TEXT NOT NULL,
    status TEXT NOT NULL,
    title TEXT NOT NULL,
    objective TEXT NOT NULL,
    session_id UUID REFERENCES chat_sessions(id) ON DELETE SET NULL,
    parent_job_id UUID REFERENCES jobs(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    error_message TEXT
);

CREATE INDEX idx_jobs_created_at ON jobs(created_at DESC);
CREATE INDEX idx_jobs_status_created_at ON jobs(status, created_at DESC);
CREATE INDEX idx_jobs_session_id ON jobs(session_id);
CREATE INDEX idx_jobs_parent_job_id ON jobs(parent_job_id);
