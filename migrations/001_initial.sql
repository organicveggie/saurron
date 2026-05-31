CREATE TABLE cycles (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    started_at   TEXT    NOT NULL,
    completed_at TEXT    NOT NULL,
    trigger      TEXT    NOT NULL,
    updated      INTEGER NOT NULL DEFAULT 0,
    rolled_back  INTEGER NOT NULL DEFAULT 0,
    failed       INTEGER NOT NULL DEFAULT 0,
    skipped      INTEGER NOT NULL DEFAULT 0,
    up_to_date   INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE cycle_containers (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    cycle_id  INTEGER NOT NULL REFERENCES cycles(id),
    name      TEXT    NOT NULL,
    old_image TEXT,
    new_image TEXT,
    outcome   TEXT    NOT NULL
);
