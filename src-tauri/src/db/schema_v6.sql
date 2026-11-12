-- Schema v6: study time, for the Insights page and the daily goal.
-- One row per item, local day and local hour, holding the seconds spent on that item's page while the window was in
-- focus and in use. The interface sends the local day and hour, so the charts line up with the learner's own calendar.
CREATE TABLE study_time (
    item_id    TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    course_id  TEXT NOT NULL REFERENCES courses(id) ON DELETE CASCADE,
    day        TEXT NOT NULL,                       -- YYYY-MM-DD, local
    hour       INTEGER NOT NULL CHECK (hour BETWEEN 0 AND 23),
    seconds    INTEGER NOT NULL DEFAULT 0 CHECK (seconds >= 0),
    PRIMARY KEY (item_id, day, hour)
);
CREATE INDEX study_time_day ON study_time(day);
