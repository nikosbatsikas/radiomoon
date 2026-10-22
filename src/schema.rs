//! Database layout. The SQL lives in the `sql/` folder and is embedded in the program:
//!   sql/schema.sql                 the complete latest layout (creates a new database)
//!   sql/migrations/NNNN_name.sql   what changes from schema NNNN-1 to NNNN (upgrades an existing database)
//!
//! To change the layout in a future version:
//!   1. edit sql/schema.sql to the new final form;
//!   2. add sql/migrations/0002_description.sql with the ALTER / CREATE statements;
//!   3. add the line to MIGRATIONS below. SCHEMA_VERSION follows automatically.

pub const SCHEMA_SQL: &str = include_str!("../sql/schema.sql");

/// (schema version, SQL). Keep them in ascending order. Version 1 is schema.sql itself.
pub const MIGRATIONS: &[(i32, &str)] = &[
    // (2, include_str!("../sql/migrations/0002_example.sql")),
];

pub const SCHEMA_VERSION: i32 = latest(MIGRATIONS, 0);

const fn latest(m: &[(i32, &str)], i: usize) -> i32 {
    // the highest migration number, at least 1
    let mut best = 1;
    let mut k = i;
    while k < m.len() {
        if m[k].0 > best {
            best = m[k].0;
        }
        k += 1;
    }
    best
}

/// Splits a SQL file into statements (a ';' at the end of a line); `--` comment lines are dropped.
pub fn statements(sql: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for line in sql.lines() {
        let t = line.trim();
        if t.starts_with("--") || (t.is_empty() && cur.is_empty()) {
            continue;
        }
        cur.push_str(line);
        cur.push('\n');
        if t.ends_with(';') {
            let q = cur.trim().trim_end_matches(';').trim().to_string();
            if !q.is_empty() {
                out.push(q);
            }
            cur.clear();
        }
    }
    let q = cur.trim();
    if !q.is_empty() {
        out.push(q.to_string());
    }
    out
}
