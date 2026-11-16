-- Schema v7: build level per course (models, effort, rewrites and reading-list length used to build it).
-- NULL means the course was built before levels existed, which matches the Standard level.
ALTER TABLE courses ADD COLUMN build_json TEXT;
