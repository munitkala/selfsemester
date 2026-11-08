-- Schema v4: strictness modes, instant feedback, review threads, and courses written by hand.

-- structured: one attempt per item, late policy applies (how every course worked before).
-- sandbox: resubmit as often as you like, no late penalties; an item closes when you reveal its answer key.
ALTER TABLE courses ADD COLUMN strictness TEXT NOT NULL DEFAULT 'structured' CHECK (strictness IN ('structured', 'sandbox'));
-- claude: planned and written by Claude. manual: written in the course builder, self-marked.
ALTER TABLE courses ADD COLUMN origin TEXT NOT NULL DEFAULT 'claude' CHECK (origin IN ('claude', 'manual'));
ALTER TABLE courses ADD COLUMN draft_json TEXT;            -- the course builder's working copy while status = 'draft'

ALTER TABLE items ADD COLUMN key_revealed INTEGER NOT NULL DEFAULT 0;  -- sandbox: answer key shown, no more attempts

-- Multiple-choice and numeric questions marked the moment work is submitted, shown while the full marking runs.
ALTER TABLE submissions ADD COLUMN instant_json TEXT;

-- Review mode: a conversation about one marked item.
CREATE TABLE review_messages (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id     TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    role        TEXT NOT NULL CHECK (role IN ('learner', 'claude', 'note')),
    text        TEXT NOT NULL,
    created_at  TEXT NOT NULL
);
CREATE INDEX review_messages_item ON review_messages(item_id, id);
