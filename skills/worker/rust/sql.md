---
name: worker/rust/sql
description: SQLite with sqlx in the Kompanion server: runtime queries, tuple rows, binds, migrations, triggers, FTS5.
roles: [worker, reviewer]
tags: [sql, sqlx, sqlite, query, migration, migrations, database, db, fts5, trigger, column, columns]
paths: ["server/migrations/**"]
---
# Worker: Rust (Kompanion server, runner, machine-stats): sql

1. (2026-10-03) SQL: use runtime queries only, `sqlx::query(...)`/`query_as(...)` with `.bind(...)`. Never the `query!` macros: they need a database at build time and the build has none. Never `format!` values into SQL.
   Example: `sqlx::query_as::<_, (String,)>("SELECT id FROM users WHERE name = ?").bind(name).fetch_optional(&db).await?`
3. (2026-10-03) The database is SQLite (`SqlitePool`). Never import Postgres types.
25. `.bind(x)` takes ownership. When the value is used again later (in the JSON answer, or a
    second query), bind a reference: `.bind(&id)`, `.bind(&u.id)`.
30. The server's database pool is `s.db` (AppState has no `pool` field).
31. Migrations of a feature built next to other work get their own number range (`0100_assets.sql`):
    sqlx applies every unapplied version in order and never checks for gaps, while two branches that
    both add `0015_*.sql` break the second deploy (checksum mismatch).
41. (2026-10-05) In SQLite triggers never rely on `INSERT OR IGNORE`: the conflict policy of the
    statement that fired the trigger wins, so an upsert (`ON CONFLICT DO UPDATE`) made the trigger's
    insert fail with a UNIQUE error. Write `INSERT ... SELECT ... WHERE NOT EXISTS (...)`.
42. (2026-10-05) FTS5 ranking costs time per match: `ORDER BY bm25(...) LIMIT 40` over 50,000
    matches took ~100 ms, without ranking 3 ms. Count the matches first and rank only up to a few
    thousand; above that, newest first (`ORDER BY rowid DESC`).
44. (2026-10-03) Annotate tuple rows: `let rows: Vec<(String, String)> = sqlx::query_as(...)`, and
    read them as `row.0`, never `row.field`.
49. (2026-10-05) A one-column `query_as` row is a tuple: `if let Some((id,)) = row`, never
    `Some(id)` (that returns `(String,)`, not `String`). Tests use only helpers that exist: a module
    such as `crate::test_helpers` is never assumed; copy the `db()`/`state()` helpers into the test
    module. Bind exactly as many values as the SQL has `?`.
54. (2026-10-06) SQL columns: use only columns you saw in the file. JSON fields of a stored column
   are read with `json_extract(tool, '$.path')`; there was no `path` column.
56. (2026-10-06) Adding columns to a `query_as` tuple changes three places together: the SELECT
   string, the tuple type and the destructuring, all appended at the end in the same order. A
   tuple that compiles but doesn't match the SELECT fails only at run time.
