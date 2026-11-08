-- Schema v5: how a course presents itself, like a real course page: course code, term, instructor, banner colour,
-- welcome message, learning outcomes and each unit's objectives (JSON, see course::CourseMeta). NULL = defaults.
ALTER TABLE courses ADD COLUMN meta_json TEXT;
