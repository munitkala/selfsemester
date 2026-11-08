-- Schema v3: the learner's own materials for a course (reference files, images, typed notes).
-- Files are copied into the course folder (courses/<id>/materials/), so removing the original doesn't break anything.

CREATE TABLE materials (
    id         TEXT PRIMARY KEY,
    course_id  TEXT NOT NULL REFERENCES courses(id) ON DELETE CASCADE,
    kind       TEXT NOT NULL CHECK (kind IN ('file', 'image', 'note')),
    name       TEXT NOT NULL,             -- shown in the app (original file name, or the note's title)
    path       TEXT NOT NULL,             -- the copy inside the course folder
    bytes      INTEGER NOT NULL DEFAULT 0,
    added_at   TEXT NOT NULL
);
CREATE INDEX materials_course ON materials(course_id, added_at);
