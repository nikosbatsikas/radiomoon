# Schema migrations

One file per schema version: `0002_add_isrc.sql`, `0003_...sql`. Each holds only the changes from the previous version
(one statement per `;` at the end of a line), for example:

```sql
ALTER TABLE tracks ADD COLUMN isrc VARCHAR(16) NOT NULL DEFAULT '';
```

Then:
1. update `sql/schema.sql` to the new final layout;
2. register the file in `src/schema.rs` (`MIGRATIONS`);
3. mention it in `CHANGELOG.md`.

On start, RadioMoon saves a safety backup (`pre-upgrade-<old>-to-<new>.rmbak`) and runs every migration newer than the
database's stored schema version. Never edit a migration that has been released.
