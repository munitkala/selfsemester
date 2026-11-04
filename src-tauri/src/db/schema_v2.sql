-- Schema v2: course pipeline state, job priority, richer items. v1 had no way to create items, submissions or
-- marks, so those tables are empty and are rebuilt here with their final shape.

-- Course pipeline state.
--   status: intake (questions being written) -> answering -> scoping -> review (readiness report shown)
--           -> compiling -> active -> completed | withdrawn ; 'failed' when a stage gave up.
ALTER TABLE courses ADD COLUMN notes TEXT;                -- extra context the user typed with the subject
ALTER TABLE courses ADD COLUMN questions_json TEXT;       -- intake questions from Claude
ALTER TABLE courses ADD COLUMN plan_json TEXT;            -- scope stage output (syllabus, pacing, readiness)
ALTER TABLE courses ADD COLUMN bundle_choice TEXT;        -- 'combined' | 'first' | 'sequence' | NULL
ALTER TABLE courses ADD COLUMN error TEXT;
ALTER TABLE courses ADD COLUMN completed_at TEXT;

ALTER TABLE jobs ADD COLUMN priority INTEGER NOT NULL DEFAULT 0;  -- higher runs first (interactive > marking > compile)
DROP INDEX jobs_runnable;
CREATE INDEX jobs_runnable ON jobs(status, priority, next_attempt_at);

ALTER TABLE units ADD COLUMN unit_key TEXT;
ALTER TABLE units ADD COLUMN week INTEGER NOT NULL DEFAULT 1;
ALTER TABLE units ADD COLUMN state TEXT NOT NULL DEFAULT 'pending';   -- pending | generated | validated
ALTER TABLE units ADD COLUMN warnings TEXT;                           -- JSON array from validation

DROP TABLE marks;
DROP TABLE submissions;
DROP TABLE items;

CREATE TABLE items (
    id            TEXT PRIMARY KEY,
    course_id     TEXT NOT NULL REFERENCES courses(id) ON DELETE CASCADE,
    unit_id       TEXT REFERENCES units(id) ON DELETE CASCADE,
    seq           INTEGER NOT NULL DEFAULT 0,  -- order inside the unit
    kind          TEXT NOT NULL CHECK (kind IN ('lesson', 'homework', 'assignment', 'quiz', 'test', 'exam', 'project')),
    title         TEXT NOT NULL,
    module        TEXT NOT NULL DEFAULT 'written',  -- written | upload | quiz | none (lessons)
    points        REAL NOT NULL DEFAULT 0,
    weight        REAL NOT NULL DEFAULT 0,          -- share of the final grade, 0..1
    release_at    TEXT NOT NULL,
    due_at        TEXT,                             -- NULL for lessons
    sealed        INTEGER NOT NULL DEFAULT 0,       -- content encrypted on disk until release
    content_ref   TEXT NOT NULL,                    -- file under the course folder (public part, or sealed file)
    key_ref       TEXT,                             -- sealed answer key + marking notes; never shown before marking
    -- locked (not released) -> open -> done (lessons read) | submitted -> marked ; missed when the deadline passes
    state         TEXT NOT NULL DEFAULT 'locked'
                  CHECK (state IN ('locked', 'open', 'done', 'submitted', 'marked', 'missed')),
    notified      TEXT NOT NULL DEFAULT '[]'        -- JSON array of toasts already sent: release, due24, due1
);
CREATE INDEX items_schedule ON items(state, release_at, due_at);
CREATE INDEX items_course ON items(course_id, release_at);

CREATE TABLE submissions (
    id           TEXT PRIMARY KEY,
    item_id      TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    submitted_at TEXT NOT NULL,            -- recorded locally, so queued marking never makes work late
    text_answer  TEXT,
    answers_json TEXT,                     -- per-question answers {question_id: answer}
    files_json   TEXT NOT NULL DEFAULT '[]',
    late_days    INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX submissions_item ON submissions(item_id);

CREATE TABLE marks (
    id            TEXT PRIMARY KEY,
    submission_id TEXT REFERENCES submissions(id) ON DELETE CASCADE, -- NULL for a missed item
    item_id       TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    raw_percent   REAL NOT NULL,           -- before late penalty
    percent       REAL NOT NULL,           -- after late penalty
    late_penalty  REAL NOT NULL DEFAULT 0, -- percentage points taken off
    breakdown     TEXT NOT NULL,           -- JSON: lines, comments, what to revise
    marker        TEXT NOT NULL,           -- 'auto' | 'missed' | model id
    ai_estimate   INTEGER NOT NULL DEFAULT 0,
    marked_at     TEXT NOT NULL
);
CREATE INDEX marks_item ON marks(item_id);
