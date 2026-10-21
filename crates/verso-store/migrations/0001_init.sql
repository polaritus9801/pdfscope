-- Foundation schema.
--
-- Both tables are an index over the disk, not a source of truth: deleting this database loses
-- nothing that cannot be rebuilt by rescanning. That constraint is what allows the schema to stay
-- small and the migrations to stay additive.
--
-- Timestamps are Unix seconds (INTEGER, UTC). The application converts; the database never guesses
-- a timezone.

CREATE TABLE books (
    id          TEXT    PRIMARY KEY,           -- lowercase hex SHA-256 of the file bytes
    path        TEXT    NOT NULL,              -- last known location, a hint only
    title       TEXT    NOT NULL,
    page_count  INTEGER NOT NULL CHECK (page_count >= 0),
    added_at    INTEGER NOT NULL
) STRICT;

CREATE TABLE reading_progress (
    book_id     TEXT    PRIMARY KEY REFERENCES books (id) ON DELETE CASCADE,
    last_page   INTEGER NOT NULL CHECK (last_page >= 0),
    updated_at  INTEGER NOT NULL
) STRICT;
