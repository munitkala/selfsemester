-- Schema v7: build level per course (models, effort, rewrites, reading-list length and what to include), and
-- pausing a course while it's being built.
-- build_json NULL means the course was built before levels existed, which matches the Standard level.
ALTER TABLE courses ADD COLUMN build_json TEXT;
-- Set while the learner has paused building: the course's build jobs are held until it's cleared.
ALTER TABLE courses ADD COLUMN paused_at TEXT;
