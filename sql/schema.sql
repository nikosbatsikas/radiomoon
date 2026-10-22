-- RadioMoon database layout, schema version 1 (latest).
-- Used to create a new database. When you change the layout: edit this file to the new final form,
-- add sql/migrations/NNNN_description.sql (changes from the previous version) and register it in src/schema.rs.
-- One statement per ';' at the end of a line. No ';' inside strings.

CREATE TABLE IF NOT EXISTS tracks(
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
            ) ENGINE=InnoDB;

CREATE TABLE IF NOT EXISTS docs(
              id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
              kind VARCHAR(24) NOT NULL,
              body LONGTEXT NOT NULL,
              KEY k_kind(kind)
            ) ENGINE=InnoDB;

CREATE TABLE IF NOT EXISTS log_entries(
              id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
              position INT NOT NULL,
              track_id BIGINT UNSIGNED NOT NULL,
              KEY k_pos(position)
            ) ENGINE=InnoDB;

CREATE TABLE IF NOT EXISTS play_history(
              id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
              track_id BIGINT UNSIGNED NOT NULL,
              played_at DATETIME DEFAULT CURRENT_TIMESTAMP,
              src VARCHAR(100) NOT NULL DEFAULT '',
              reason VARCHAR(1000) NOT NULL DEFAULT '',
              KEY k_track(track_id),
              KEY k_time(played_at)
            ) ENGINE=InnoDB;

CREATE TABLE IF NOT EXISTS settings(
              k VARCHAR(64) PRIMARY KEY,
              v LONGTEXT NOT NULL
            ) ENGINE=InnoDB;
