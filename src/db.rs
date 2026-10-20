use crate::config::Config;
use crate::model::*;
use anyhow::{bail, Result};
use std::path::Path;
use mysql::{params, prelude::*, OptsBuilder, Pool, Row};
use serde::{de::DeserializeOwned, Serialize};

#[derive(Clone)]
pub struct Db {
    pub(crate) pool: Pool,
}

/// Version of the database layout. Increase it when a migration is added to MIGRATIONS.
pub const SCHEMA_VERSION: i32 = 1;

/// Upgrade steps: (schema version, SQL statements that bring an older database to that version).
/// Example for a future release:  (2, &["ALTER TABLE tracks ADD COLUMN isrc VARCHAR(16) NOT NULL DEFAULT ''"]),
const MIGRATIONS: &[(i32, &[&str])] = &[];

pub struct HistRow {
    pub track_id: u64,
    pub at: String,
    pub artist: String,
    pub title: String,
    pub kind: String,
    pub src: String,
    pub reason: String,
}

const TCOLS: &str = "id,kind,title,artist,album,genre,year,path,duration_ms,cat1,cat2,cat3,vocal,props,enabled,bookmarked,intro_ms,mix_in_ms,mix_out_ms,hook_in_ms,hook_out_ms,gain_db,sep_min,plcode,notes,variable_len";

fn track_row(mut r: Row) -> Track {
    let g: f64 = r.take("gain_db").unwrap_or(0.0);
    Track {
        id: r.take("id").unwrap_or(0),
        kind: r.take("kind").unwrap_or_default(),
        title: r.take("title").unwrap_or_default(),
        artist: r.take("artist").unwrap_or_default(),
        album: r.take("album").unwrap_or_default(),
        genre: r.take("genre").unwrap_or_default(),
        year: r.take("year").unwrap_or_default(),
        path: r.take("path").unwrap_or_default(),
        duration_ms: r.take("duration_ms").unwrap_or(0),
        cat1: r.take("cat1").unwrap_or_default(),
        cat2: r.take("cat2").unwrap_or_default(),
        cat3: r.take("cat3").unwrap_or_default(),
        vocal: r.take("vocal").unwrap_or_default(),
        props: r.take("props").unwrap_or_default(),
        enabled: r.take("enabled").unwrap_or(true),
        bookmarked: r.take("bookmarked").unwrap_or(false),
        intro_ms: r.take("intro_ms").unwrap_or(0),
        mix_in_ms: r.take("mix_in_ms").unwrap_or(0),
        mix_out_ms: r.take("mix_out_ms").unwrap_or(0),
        hook_in_ms: r.take("hook_in_ms").unwrap_or(0),
        hook_out_ms: r.take("hook_out_ms").unwrap_or(0),
        gain_db: g as f32,
        sep_min: r.take("sep_min").unwrap_or(0),
        plcode: r.take("plcode").unwrap_or_default(),
        notes: r.take("notes").unwrap_or_default(),
        variable_len: r.take("variable_len").unwrap_or(false),
        ..Default::default()
    }
}

impl Db {
    pub fn connect(cfg: &Config) -> Result<Self> {
        let base = || {
            OptsBuilder::new()
                .ip_or_hostname(Some(cfg.host.clone()))
                .tcp_port(cfg.port)
                .user(Some(cfg.user.clone()))
                .pass(if cfg.password.is_empty() { None } else { Some(cfg.password.clone()) })
        };
        let admin = Pool::new(base())?;
        admin.get_conn()?.query_drop(format!(
            "CREATE DATABASE IF NOT EXISTS `{}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci",
            cfg.database
        ))?;
        let pool = Pool::new(base().db_name(Some(cfg.database.clone())))?;
        let db = Db { pool };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        let mut c = self.pool.get_conn()?;
        c.query_drop(
            "CREATE TABLE IF NOT EXISTS tracks(
              id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
              kind VARCHAR(16) NOT NULL,
              title VARCHAR(255) NOT NULL,
              artist VARCHAR(255) NOT NULL DEFAULT '',
              album VARCHAR(255) NOT NULL DEFAULT '',
              genre VARCHAR(100) NOT NULL DEFAULT '',
              year VARCHAR(8) NOT NULL DEFAULT '',
              path VARCHAR(512) NOT NULL,
              duration_ms BIGINT UNSIGNED NOT NULL DEFAULT 0,
              cat1 VARCHAR(100) NOT NULL DEFAULT '',
              cat2 VARCHAR(100) NOT NULL DEFAULT '',
              cat3 VARCHAR(100) NOT NULL DEFAULT '',
              vocal VARCHAR(100) NOT NULL DEFAULT '',
              props VARCHAR(100) NOT NULL DEFAULT '',
              enabled TINYINT(1) NOT NULL DEFAULT 1,
              bookmarked TINYINT(1) NOT NULL DEFAULT 0,
              intro_ms BIGINT UNSIGNED NOT NULL DEFAULT 0,
              mix_in_ms BIGINT UNSIGNED NOT NULL DEFAULT 0,
              mix_out_ms BIGINT UNSIGNED NOT NULL DEFAULT 0,
              hook_in_ms BIGINT UNSIGNED NOT NULL DEFAULT 0,
              hook_out_ms BIGINT UNSIGNED NOT NULL DEFAULT 0,
              gain_db DOUBLE NOT NULL DEFAULT 0,
              sep_min INT UNSIGNED NOT NULL DEFAULT 0,
              plcode VARCHAR(64) NOT NULL DEFAULT '',
              notes VARCHAR(1000) NOT NULL DEFAULT '',
              variable_len TINYINT(1) NOT NULL DEFAULT 0,
              created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
              UNIQUE KEY uq_path(path),
              KEY k_kind(kind)
            ) ENGINE=InnoDB",
        )?;
        c.query_drop(
            "CREATE TABLE IF NOT EXISTS docs(
              id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
              kind VARCHAR(24) NOT NULL,
              body LONGTEXT NOT NULL,
              KEY k_kind(kind)
            ) ENGINE=InnoDB",
        )?;
        c.query_drop(
            "CREATE TABLE IF NOT EXISTS log_entries(
              id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
              position INT NOT NULL,
              track_id BIGINT UNSIGNED NOT NULL,
              KEY k_pos(position)
            ) ENGINE=InnoDB",
        )?;
        c.query_drop(
            "CREATE TABLE IF NOT EXISTS play_history(
              id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
              track_id BIGINT UNSIGNED NOT NULL,
              played_at DATETIME DEFAULT CURRENT_TIMESTAMP,
              src VARCHAR(100) NOT NULL DEFAULT '',
              reason VARCHAR(1000) NOT NULL DEFAULT '',
              KEY k_track(track_id),
              KEY k_time(played_at)
            ) ENGINE=InnoDB",
        )?;
        c.query_drop(
            "CREATE TABLE IF NOT EXISTS settings(
              k VARCHAR(64) PRIMARY KEY,
              v LONGTEXT NOT NULL
            ) ENGINE=InnoDB",
        )?;
        Ok(())
    }

    // ---------- tracks ----------
    pub fn list_tracks(&self, kind: Option<&str>) -> Result<Vec<Track>> {
        let mut c = self.pool.get_conn()?;
        let rows: Vec<Row> = match kind {
            Some(k) => c.exec(format!("SELECT {TCOLS} FROM tracks WHERE kind=? ORDER BY artist,title"), (k,))?,
            None => c.query(format!("SELECT {TCOLS} FROM tracks ORDER BY kind,artist,title"))?,
        };
        Ok(rows.into_iter().map(track_row).collect())
    }

    /// Inserts a new track or refreshes the metadata of an existing one (same path).
    pub fn insert_track(&self, t: &Track) -> Result<()> {
        let mut c = self.pool.get_conn()?;
        c.exec_drop(
            "INSERT INTO tracks(kind,title,artist,album,genre,year,path,duration_ms,cat1,cat2,cat3,vocal,props,enabled,bookmarked,
               intro_ms,mix_in_ms,mix_out_ms,hook_in_ms,hook_out_ms,gain_db,sep_min,plcode,notes,variable_len)
             VALUES(:kind,:title,:artist,:album,:genre,:year,:path,:dur,:cat1,:cat2,:cat3,:vocal,:props,:enabled,:bm,
               :intro,:mixin,:mixout,:hookin,:hookout,:gain,:sep,:plcode,:notes,:var)
             ON DUPLICATE KEY UPDATE title=VALUES(title),artist=VALUES(artist),album=VALUES(album),genre=VALUES(genre),
               year=VALUES(year),duration_ms=VALUES(duration_ms)",
            params! {
                "kind" => t.kind.clone(), "title" => t.title.clone(), "artist" => t.artist.clone(), "album" => t.album.clone(),
                "genre" => t.genre.clone(), "year" => t.year.clone(), "path" => t.path.clone(), "dur" => t.duration_ms,
                "cat1" => t.cat1.clone(), "cat2" => t.cat2.clone(), "cat3" => t.cat3.clone(), "vocal" => t.vocal.clone(),
                "props" => t.props.clone(), "enabled" => t.enabled, "bm" => t.bookmarked, "intro" => t.intro_ms,
                "mixin" => t.mix_in_ms, "mixout" => t.mix_out_ms, "hookin" => t.hook_in_ms, "hookout" => t.hook_out_ms,
                "gain" => t.gain_db as f64, "sep" => t.sep_min, "plcode" => t.plcode.clone(), "notes" => t.notes.clone(),
                "var" => t.variable_len,
            },
        )?;
        Ok(())
    }

    pub fn update_track(&self, t: &Track) -> Result<()> {
        let mut c = self.pool.get_conn()?;
        c.exec_drop(
            "UPDATE tracks SET kind=:kind,title=:title,artist=:artist,album=:album,genre=:genre,year=:year,path=:path,
               duration_ms=:dur,cat1=:cat1,cat2=:cat2,cat3=:cat3,vocal=:vocal,props=:props,enabled=:enabled,bookmarked=:bm,
               intro_ms=:intro,mix_in_ms=:mixin,mix_out_ms=:mixout,hook_in_ms=:hookin,hook_out_ms=:hookout,gain_db=:gain,
               sep_min=:sep,plcode=:plcode,notes=:notes,variable_len=:var WHERE id=:id",
            params! {
                "id" => t.id, "kind" => t.kind.clone(), "title" => t.title.clone(), "artist" => t.artist.clone(),
                "album" => t.album.clone(), "genre" => t.genre.clone(), "year" => t.year.clone(), "path" => t.path.clone(),
                "dur" => t.duration_ms, "cat1" => t.cat1.clone(), "cat2" => t.cat2.clone(), "cat3" => t.cat3.clone(),
                "vocal" => t.vocal.clone(), "props" => t.props.clone(), "enabled" => t.enabled, "bm" => t.bookmarked,
                "intro" => t.intro_ms, "mixin" => t.mix_in_ms, "mixout" => t.mix_out_ms, "hookin" => t.hook_in_ms,
                "hookout" => t.hook_out_ms, "gain" => t.gain_db as f64, "sep" => t.sep_min, "plcode" => t.plcode.clone(),
                "notes" => t.notes.clone(), "var" => t.variable_len,
            },
        )?;
        Ok(())
    }

    pub fn delete_tracks(&self, ids: &[u64]) -> Result<()> {
        let mut c = self.pool.get_conn()?;
        for id in ids {
            c.exec_drop("DELETE FROM tracks WHERE id=?", (*id,))?;
            c.exec_drop("DELETE FROM log_entries WHERE track_id=?", (*id,))?;
        }
        Ok(())
    }

    pub fn reset_kinds(&self, kinds: &[String]) -> Result<()> {
        let mut c = self.pool.get_conn()?;
        for k in kinds {
            c.exec_drop("DELETE FROM tracks WHERE kind=?", (k.as_str(),))?;
        }
        c.query_drop("DELETE FROM log_entries WHERE track_id NOT IN (SELECT id FROM tracks)")?;
        Ok(())
    }

    // ---------- generic documents (clocks, campaigns, playlists ...) ----------
    pub fn docs<T: DeserializeOwned + HasId>(&self, kind: &str) -> Result<Vec<T>> {
        let mut c = self.pool.get_conn()?;
        let rows: Vec<(u64, String)> = c.exec("SELECT id,body FROM docs WHERE kind=? ORDER BY id", (kind,))?;
        let mut v = Vec::new();
        for (id, b) in rows {
            if let Ok(mut x) = serde_json::from_str::<T>(&b) {
                x.set_id(id);
                v.push(x);
            }
        }
        Ok(v)
    }

    pub fn save_doc<T: Serialize + HasId>(&self, kind: &str, x: &mut T) -> Result<()> {
        let body = serde_json::to_string(x)?;
        let mut c = self.pool.get_conn()?;
        if x.id() == 0 {
            c.exec_drop("INSERT INTO docs(kind,body) VALUES(?,?)", (kind, body))?;
            x.set_id(c.last_insert_id());
        } else {
            c.exec_drop("UPDATE docs SET body=? WHERE id=? AND kind=?", (body, x.id(), kind))?;
        }
        Ok(())
    }

    pub fn del_doc(&self, id: u64) -> Result<()> {
        let mut c = self.pool.get_conn()?;
        c.exec_drop("DELETE FROM docs WHERE id=?", (id,))?;
        Ok(())
    }

    // ---------- settings ----------
    pub fn load_settings(&self) -> Settings {
        self.get_setting("app")
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save_settings(&self, s: &Settings) {
        if let Ok(js) = serde_json::to_string(s) {
            let _ = self.set_setting("app", &js);
        }
    }

    pub fn get_setting(&self, k: &str) -> Result<Option<String>> {
        let mut c = self.pool.get_conn()?;
        Ok(c.exec_first("SELECT v FROM settings WHERE k=?", (k,))?)
    }

    pub fn set_setting(&self, k: &str, v: &str) -> Result<()> {
        let mut c = self.pool.get_conn()?;
        c.exec_drop("REPLACE INTO settings(k,v) VALUES(?,?)", (k, v))?;
        Ok(())
    }

    // ---------- current playout log ----------
    pub fn save_log(&self, ids: &[u64]) -> Result<()> {
        let mut c = self.pool.get_conn()?;
        c.query_drop("DELETE FROM log_entries")?;
        c.exec_batch(
            "INSERT INTO log_entries(position,track_id) VALUES(?,?)",
            ids.iter().enumerate().map(|(i, id)| (i as u32, *id)),
        )?;
        Ok(())
    }

    pub fn load_log(&self) -> Result<Vec<Track>> {
        let cols: Vec<String> = TCOLS.split(',').map(|c| format!("t.{c}")).collect();
        let mut c = self.pool.get_conn()?;
        let rows: Vec<Row> = c.query(format!(
            "SELECT {} FROM log_entries l JOIN tracks t ON t.id=l.track_id ORDER BY l.position",
            cols.join(",")
        ))?;
        Ok(rows.into_iter().map(track_row).collect())
    }

    // ---------- history ----------
    pub fn mark_played(&self, track_id: u64, src: &str, reason: &str) -> Result<()> {
        let mut c = self.pool.get_conn()?;
        c.exec_drop("INSERT INTO play_history(track_id,src,reason) VALUES(?,?,?)", (track_id, src, reason))?;
        Ok(())
    }

    pub fn history(&self, limit: u32) -> Result<Vec<(String, String)>> {
        let mut c = self.pool.get_conn()?;
        let rows: Vec<(String, String, String)> = c.query(format!(
            "SELECT DATE_FORMAT(h.played_at,'%H:%i:%s'),t.artist,t.title
             FROM play_history h JOIN tracks t ON t.id=h.track_id ORDER BY h.id DESC LIMIT {limit}"
        ))?;
        Ok(rows.into_iter().map(|(tm, a, t)| (tm, if a.is_empty() { t } else { format!("{a} - {t}") })).collect())
    }

    pub fn history_range(&self, from: &str, to: &str) -> Result<Vec<HistRow>> {
        let mut c = self.pool.get_conn()?;
        let rows: Vec<(u64, u64, String, String, String, String, String, String)> = c.exec(
            "SELECT h.id,h.track_id,DATE_FORMAT(h.played_at,'%Y-%m-%d %H:%i:%s'),COALESCE(t.artist,''),COALESCE(t.title,'(deleted)'),
                    COALESCE(t.kind,''),h.src,h.reason
             FROM play_history h LEFT JOIN tracks t ON t.id=h.track_id
             WHERE DATE(h.played_at) BETWEEN ? AND ? ORDER BY h.id DESC LIMIT 20000",
            (from, to),
        )?;
        Ok(rows
            .into_iter()
            .map(|(_id, track_id, at, artist, title, kind, src, reason)| HistRow { track_id, at, artist, title, kind, src, reason })
            .collect())
    }

    pub fn recent_plays(&self, hours: u32) -> Result<Vec<(u64, String)>> {
        let mut c = self.pool.get_conn()?;
        Ok(c.exec(
            "SELECT track_id,DATE_FORMAT(played_at,'%Y-%m-%d %H:%i:%s') FROM play_history WHERE played_at > (NOW() - INTERVAL ? HOUR)",
            (hours,),
        )?)
    }

    pub fn play_counts(&self, from: &str, to: &str) -> Result<Vec<(u64, u64)>> {
        let mut c = self.pool.get_conn()?;
        Ok(c.exec(
            "SELECT track_id,COUNT(*) FROM play_history WHERE DATE(played_at) BETWEEN ? AND ? GROUP BY track_id",
            (from, to),
        )?)
    }

    pub fn cleanup_history(&self, days: u32) -> Result<()> {
        let mut c = self.pool.get_conn()?;
        c.exec_drop("DELETE FROM play_history WHERE played_at < (NOW() - INTERVAL ? DAY)", (days,))?;
        let _ = c.query_drop("OPTIMIZE TABLE play_history");
        Ok(())
    }

    // ---------- versions, upgrades ----------
    pub fn count_tracks(&self) -> u64 {
        self.pool
            .get_conn()
            .ok()
            .and_then(|mut c| c.query_first::<u64, _>("SELECT COUNT(*) FROM tracks").ok().flatten())
            .unwrap_or(0)
    }

    /// Compares the stored application / schema version with this program. When the program is newer
    /// it first writes a safety backup, then runs the migrations (no data is lost). Returns a message.
    pub fn ensure_schema(&self, app_version: &str, backup_dir: &Path) -> Result<String> {
        let stored_schema: i32 = self.get_setting("schema_version")?.and_then(|s| s.parse().ok()).unwrap_or(0);
        let stored_app = self.get_setting("app_version")?.unwrap_or_default();
        if stored_schema > SCHEMA_VERSION {
            bail!(
                "This database was written by a newer version of RadioMoon ({stored_app}, schema {stored_schema}); this program ({app_version}) only knows schema {SCHEMA_VERSION}. Install the newer version."
            );
        }
        let mut msg = String::new();
        if stored_app != app_version || stored_schema < SCHEMA_VERSION {
            let upgrade = !stored_app.is_empty();
            if upgrade && self.count_tracks() > 0 {
                let _ = std::fs::create_dir_all(backup_dir);
                let file = backup_dir.join(format!("pre-upgrade-{stored_app}-to-{app_version}.rmbak"));
                let _ = crate::backup::export(self, &file, &stored_app);
            }
            let mut c = self.pool.get_conn()?;
            for (v, sqls) in MIGRATIONS {
                if *v > stored_schema {
                    for q in sqls.iter() {
                        c.query_drop(*q)?;
                    }
                }
            }
            if upgrade {
                self.set_setting("previous_app_version", &stored_app)?;
                msg = format!("Database upgraded from version {stored_app} to {app_version}; a safety backup was saved.");
            }
        }
        self.set_setting("schema_version", &SCHEMA_VERSION.to_string())?;
        self.set_setting("app_version", app_version)?;
        Ok(msg)
    }

    // ---------- backup / restore helpers ----------
    pub fn all_docs(&self) -> Result<Vec<(u64, String, String)>> {
        let mut c = self.pool.get_conn()?;
        Ok(c.query("SELECT id,kind,body FROM docs ORDER BY id")?)
    }

    pub fn all_settings(&self) -> Result<Vec<(String, String)>> {
        let mut c = self.pool.get_conn()?;
        Ok(c.query("SELECT k,v FROM settings")?)
    }

    pub fn all_history(&self) -> Result<Vec<(u64, u64, String, String, String)>> {
        let mut c = self.pool.get_conn()?;
        Ok(c.query("SELECT id,track_id,DATE_FORMAT(played_at,'%Y-%m-%d %H:%i:%s'),src,reason FROM play_history ORDER BY id")?)
    }

    pub fn log_ids(&self) -> Result<Vec<u64>> {
        let mut c = self.pool.get_conn()?;
        Ok(c.query("SELECT track_id FROM log_entries ORDER BY position")?)
    }

    pub fn wipe_all(&self) -> Result<()> {
        let mut c = self.pool.get_conn()?;
        for t in ["tracks", "docs", "log_entries", "play_history", "settings"] {
            c.query_drop(format!("DELETE FROM {t}"))?;
        }
        Ok(())
    }

    pub fn restore_track(&self, t: &Track) -> Result<()> {
        let mut c = self.pool.get_conn()?;
        c.exec_drop(
            "INSERT INTO tracks(id,kind,title,artist,album,genre,year,path,duration_ms,cat1,cat2,cat3,vocal,props,enabled,bookmarked,
               intro_ms,mix_in_ms,mix_out_ms,hook_in_ms,hook_out_ms,gain_db,sep_min,plcode,notes,variable_len)
             VALUES(:id,:kind,:title,:artist,:album,:genre,:year,:path,:dur,:cat1,:cat2,:cat3,:vocal,:props,:enabled,:bm,
               :intro,:mixin,:mixout,:hookin,:hookout,:gain,:sep,:plcode,:notes,:var)",
            params! {
                "id" => t.id, "kind" => t.kind.clone(), "title" => t.title.clone(), "artist" => t.artist.clone(),
                "album" => t.album.clone(), "genre" => t.genre.clone(), "year" => t.year.clone(), "path" => t.path.clone(),
                "dur" => t.duration_ms, "cat1" => t.cat1.clone(), "cat2" => t.cat2.clone(), "cat3" => t.cat3.clone(),
                "vocal" => t.vocal.clone(), "props" => t.props.clone(), "enabled" => t.enabled, "bm" => t.bookmarked,
                "intro" => t.intro_ms, "mixin" => t.mix_in_ms, "mixout" => t.mix_out_ms, "hookin" => t.hook_in_ms,
                "hookout" => t.hook_out_ms, "gain" => t.gain_db as f64, "sep" => t.sep_min, "plcode" => t.plcode.clone(),
                "notes" => t.notes.clone(), "var" => t.variable_len,
            },
        )?;
        Ok(())
    }

    pub fn restore_docs(&self, docs: &[(u64, String, String)]) -> Result<()> {
        let mut c = self.pool.get_conn()?;
        c.exec_batch("INSERT INTO docs(id,kind,body) VALUES(?,?,?)", docs.iter().map(|d| (d.0, d.1.clone(), d.2.clone())))?;
        Ok(())
    }

    pub fn restore_settings(&self, st: &[(String, String)]) -> Result<()> {
        let mut c = self.pool.get_conn()?;
        c.exec_batch("REPLACE INTO settings(k,v) VALUES(?,?)", st.iter().map(|d| (d.0.clone(), d.1.clone())))?;
        Ok(())
    }

    pub fn restore_history(&self, h: &[(u64, u64, String, String, String)]) -> Result<()> {
        let mut c = self.pool.get_conn()?;
        for chunk in h.chunks(2000) {
            c.exec_batch(
                "INSERT INTO play_history(id,track_id,played_at,src,reason) VALUES(?,?,?,?,?)",
                chunk.iter().map(|r| (r.0, r.1, r.2.clone(), r.3.clone(), r.4.clone())),
            )?;
        }
        Ok(())
    }

    pub fn shutdown(&self) -> Result<()> {
        let mut c = self.pool.get_conn()?;
        let _ = c.query_drop("SHUTDOWN");
        Ok(())
    }
}
