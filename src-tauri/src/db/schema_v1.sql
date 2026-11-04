-- SelfSemester schema, version 1.
-- Times are UTC, stored as ISO-8601 text ("2026-11-04T15:00:00Z"). JSON columns hold validated JSON text.

CREATE TABLE profiles (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL,
    grading_scale TEXT NOT NULL DEFAULT 'percent_letter_gpa4',
    created_at    TEXT NOT NULL
);

-- Domain profiles live as JSON files in domains\; this is the registry.
CREATE TABLE domain_profiles (
    id         TEXT PRIMARY KEY,           -- e.g. 'theory-maths'
    title      TEXT NOT NULL,
    file_name  TEXT NOT NULL,
    version    INTEGER NOT NULL DEFAULT 1,
    approved   INTEGER NOT NULL DEFAULT 0, -- user approved a Claude-drafted profile
    updated_at TEXT NOT NULL
);

CREATE TABLE courses (
    id             TEXT PRIMARY KEY,
    profile_id     TEXT NOT NULL REFERENCES profiles(id),
    domain_id      TEXT REFERENCES domain_profiles(id),
    title          TEXT NOT NULL,
    subject        TEXT NOT NULL,          -- what the user asked for
    -- drafting -> compiling -> ready (sealed) -> active -> completed | withdrawn
    status         TEXT NOT NULL DEFAULT 'drafting',
    mode           TEXT NOT NULL DEFAULT 'offline' CHECK (mode IN ('offline', 'online')),
    late_policy    TEXT NOT NULL DEFAULT '{"kind":"lock_zero"}',
    on_transcript  INTEGER NOT NULL DEFAULT 1,
    intake_json    TEXT,                   -- answers from the intake stage
    readiness_json TEXT,                   -- accuracy / readiness report
    starts_at      TEXT,
    ends_at        TEXT,
    final_percent  REAL,
    created_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL
);

-- Queued Claude Code jobs (every compile stage and every marking call). Checkpointed so nothing paid for in
-- usage runs twice. 'waiting_usage' = the plan's usage limit was hit; retry at next_attempt_at.
CREATE TABLE jobs (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    course_id       TEXT REFERENCES courses(id) ON DELETE CASCADE,
    kind            TEXT NOT NULL,          -- intake | scope | domain | sources | map | unit | validate | mark | test
    unit_key        TEXT,                   -- for per-unit jobs
    status          TEXT NOT NULL DEFAULT 'queued'
                    CHECK (status IN ('queued', 'running', 'waiting_usage', 'done', 'failed', 'cancelled')),
    model           TEXT NOT NULL,
    request_json    TEXT NOT NULL,          -- prompt, schema, tools
    result_json     TEXT,
    error           TEXT,
    attempts        INTEGER NOT NULL DEFAULT 0,
    next_attempt_at TEXT,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);
CREATE INDEX jobs_runnable ON jobs(status, next_attempt_at);

-- One row per Claude Code call, for the usage view. Never stores credentials.
CREATE TABLE claude_calls (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id        INTEGER REFERENCES jobs(id) ON DELETE SET NULL,
    model         TEXT NOT NULL,
    input_tokens  INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    cache_tokens  INTEGER NOT NULL DEFAULT 0,
    duration_ms   INTEGER NOT NULL DEFAULT 0,
    outcome       TEXT NOT NULL,            -- ok | usage_limited | error
    at            TEXT NOT NULL
);

CREATE TABLE units (
    id         TEXT PRIMARY KEY,
    course_id  TEXT NOT NULL REFERENCES courses(id) ON DELETE CASCADE,
    seq        INTEGER NOT NULL,
    title      TEXT NOT NULL,
    summary    TEXT,
    UNIQUE (course_id, seq)
);

-- Everything released on a schedule: lessons, homework, assignments, quizzes, tests, exams, projects.
CREATE TABLE items (
    id          TEXT PRIMARY KEY,
    course_id   TEXT NOT NULL REFERENCES courses(id) ON DELETE CASCADE,
    unit_id     TEXT REFERENCES units(id) ON DELETE CASCADE,
    kind        TEXT NOT NULL CHECK (kind IN ('lesson', 'homework', 'assignment', 'quiz', 'test', 'exam', 'project')),
    title       TEXT NOT NULL,
    module      TEXT,                       -- capability module used to submit (written, upload, code, audio, camera, quiz)
    weight      REAL NOT NULL DEFAULT 0,    -- share of the final grade, 0..1
    release_at  TEXT NOT NULL,
    due_at      TEXT,
    sealed      INTEGER NOT NULL DEFAULT 0, -- content encrypted until release_at
    content_ref TEXT,                       -- path under the course folder
    rubric_ref  TEXT,
    -- locked (not yet released) -> open -> submitted -> marked | missed
    state       TEXT NOT NULL DEFAULT 'locked'
                CHECK (state IN ('locked', 'open', 'submitted', 'marked', 'missed')),
    depends_on  TEXT                        -- JSON array of item ids
);
CREATE INDEX items_schedule ON items(state, release_at, due_at);

CREATE TABLE submissions (
    id           TEXT PRIMARY KEY,
    item_id      TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    submitted_at TEXT NOT NULL,             -- recorded locally, so offline queueing never makes work late
    files_json   TEXT NOT NULL DEFAULT '[]',
    text_answer  TEXT,
    sent_at      TEXT                       -- when it was handed to marking
);

CREATE TABLE marks (
    id            TEXT PRIMARY KEY,
    submission_id TEXT NOT NULL REFERENCES submissions(id) ON DELETE CASCADE,
    percent       REAL NOT NULL,
    breakdown     TEXT NOT NULL,            -- JSON: score per rubric line, comments, what to revise
    model         TEXT,                     -- NULL for local auto-marking
    is_remark     INTEGER NOT NULL DEFAULT 0,
    ai_estimate   INTEGER NOT NULL DEFAULT 0, -- step rated below Medium accuracy
    late_penalty  REAL NOT NULL DEFAULT 0,
    marked_at     TEXT NOT NULL
);

CREATE TABLE sources (
    id          TEXT PRIMARY KEY,           -- content hash
    course_id   TEXT NOT NULL REFERENCES courses(id) ON DELETE CASCADE,
    url         TEXT NOT NULL,
    title       TEXT,
    licence     TEXT,
    attribution TEXT,
    free        INTEGER NOT NULL DEFAULT 1, -- 0 = "get this yourself"
    file_ref    TEXT,
    text_ref    TEXT,
    fetched_at  TEXT
);

CREATE TABLE chunks (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    source_id TEXT NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
    seq       INTEGER NOT NULL,
    page      INTEGER,
    text      TEXT NOT NULL,
    topics    TEXT                          -- JSON array of syllabus topic keys (stage 6)
);

CREATE VIRTUAL TABLE chunks_fts USING fts5(text, content='chunks', content_rowid='id');
CREATE TRIGGER chunks_ai AFTER INSERT ON chunks BEGIN
    INSERT INTO chunks_fts(rowid, text) VALUES (new.id, new.text);
END;
CREATE TRIGGER chunks_ad AFTER DELETE ON chunks BEGIN
    INSERT INTO chunks_fts(chunks_fts, rowid, text) VALUES ('delete', old.id, old.text);
END;
CREATE TRIGGER chunks_au AFTER UPDATE OF text ON chunks BEGIN
    INSERT INTO chunks_fts(chunks_fts, rowid, text) VALUES ('delete', old.id, old.text);
    INSERT INTO chunks_fts(rowid, text) VALUES (new.id, new.text);
END;

CREATE TABLE transcript_entries (
    course_id     TEXT PRIMARY KEY REFERENCES courses(id) ON DELETE CASCADE,
    profile_id    TEXT NOT NULL REFERENCES profiles(id),
    title         TEXT NOT NULL,
    started_on    TEXT NOT NULL,
    finished_on   TEXT NOT NULL,
    study_hours   REAL,
    final_percent REAL NOT NULL,
    letter        TEXT NOT NULL,
    breakdown     TEXT NOT NULL             -- JSON: assignments / tests / projects / exams
);

CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
