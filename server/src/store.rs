//! SQLite store: players, scores (with the replay), one table each.

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

pub struct Store {
    pub conn: Connection,
}

#[derive(Clone, Debug, Serialize)]
pub struct Entry {
    pub rank: u32,
    pub id: i64,
    pub player: String,
    pub name: String,
    pub score: i64,
    pub pp: f64,
    pub grade: String,
    pub fc: bool,
    pub pfc: bool,
    pub max_combo: i64,
    pub beat_accuracy: f64,
    pub timing_std_ms: f64,
    pub downs: i64,
    pub clean: bool,
    pub flags: Vec<String>,
    pub when: String,
}

#[derive(Clone, Debug)]
pub struct NewScore<'a> {
    pub player: &'a str,
    pub fingerprint: u64,
    pub mods: &'a str,
    pub level: &'a str,
    pub bpm: f64,
    pub score: i64,
    pub pp: f64,
    pub grade: &'a str,
    pub fc: bool,
    pub pfc: bool,
    pub max_combo: i64,
    pub beat_accuracy: f64,
    pub timing_std_ms: f64,
    pub downs: i64,
    pub clean: bool,
    pub flags: &'a [String],
    pub ranked: bool,
    pub complete: bool,
    pub daily: Option<&'a str>,
    pub replay: &'a str,
}

pub fn now_iso() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) as i64;
    let day = crate::daily::civil_from_days(secs.div_euclid(86400));
    let s = secs.rem_euclid(86400);
    format!("{day}T{:02}:{:02}:{:02}Z", s / 3600, (s / 60) % 60, s % 60)
}

impl Store {
    pub fn open(path: &str) -> rusqlite::Result<Store> {
        let conn = if path == ":memory:" { Connection::open_in_memory()? } else { Connection::open(path)? };
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS players (
                 id TEXT PRIMARY KEY, token_hash TEXT NOT NULL, name TEXT NOT NULL, created TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS scores (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 player TEXT NOT NULL, fingerprint TEXT NOT NULL, mods TEXT NOT NULL,
                 level TEXT NOT NULL, bpm REAL NOT NULL,
                 score INTEGER NOT NULL, pp REAL NOT NULL, grade TEXT NOT NULL,
                 fc INTEGER NOT NULL, pfc INTEGER NOT NULL, max_combo INTEGER NOT NULL,
                 beat_accuracy REAL NOT NULL, timing_std_ms REAL NOT NULL, downs INTEGER NOT NULL,
                 clean INTEGER NOT NULL, flags TEXT NOT NULL, ranked INTEGER NOT NULL, complete INTEGER NOT NULL,
                 daily TEXT, replay TEXT NOT NULL, created TEXT NOT NULL);
             CREATE INDEX IF NOT EXISTS scores_board ON scores (fingerprint, mods, daily, score DESC);
             CREATE INDEX IF NOT EXISTS scores_daily ON scores (daily, score DESC);
             CREATE INDEX IF NOT EXISTS scores_player ON scores (player, fingerprint);",
        )?;
        Ok(Store { conn })
    }

    pub fn create_player(&self, id: &str, token_hash: &str, name: &str) -> rusqlite::Result<()> {
        self.conn.execute("INSERT INTO players (id, token_hash, name, created) VALUES (?1, ?2, ?3, ?4)", params![id, token_hash, name, now_iso()])?;
        Ok(())
    }

    pub fn player_token_hash(&self, id: &str) -> rusqlite::Result<Option<String>> {
        self.conn.query_row("SELECT token_hash FROM players WHERE id = ?1", params![id], |r| r.get(0)).optional()
    }

    pub fn player_name(&self, id: &str) -> rusqlite::Result<Option<String>> {
        self.conn.query_row("SELECT name FROM players WHERE id = ?1", params![id], |r| r.get(0)).optional()
    }

    pub fn rename_player(&self, id: &str, name: &str) -> rusqlite::Result<()> {
        self.conn.execute("UPDATE players SET name = ?2 WHERE id = ?1", params![id, name])?;
        Ok(())
    }

    pub fn insert_score(&self, s: &NewScore) -> rusqlite::Result<i64> {
        self.conn.execute(
            "INSERT INTO scores (player, fingerprint, mods, level, bpm, score, pp, grade, fc, pfc, max_combo, beat_accuracy, timing_std_ms, downs, clean, flags, ranked, complete, daily, replay, created)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)",
            params![
                s.player,
                format!("{:016x}", s.fingerprint),
                s.mods,
                s.level,
                s.bpm,
                s.score,
                s.pp,
                s.grade,
                s.fc as i64,
                s.pfc as i64,
                s.max_combo,
                s.beat_accuracy,
                s.timing_std_ms,
                s.downs,
                s.clean as i64,
                serde_json::to_string(s.flags).unwrap_or_else(|_| "[]".into()),
                s.ranked as i64,
                s.complete as i64,
                s.daily,
                s.replay,
                now_iso()
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// A board: best complete score per player, clean first, then by score.
    /// `daily` selects the daily board for that date (fingerprint then ignored).
    pub fn board(&self, fingerprint: Option<u64>, mods: &str, daily: Option<&str>, limit: u32) -> rusqlite::Result<Vec<Entry>> {
        let fp = fingerprint.map(|f| format!("{f:016x}")).unwrap_or_default();
        let sql = "SELECT s.id, s.player, p.name, s.score, s.pp, s.grade, s.fc, s.pfc, s.max_combo, s.beat_accuracy, s.timing_std_ms, s.downs, s.clean, s.flags, s.created
                   FROM scores s JOIN players p ON p.id = s.player
                   WHERE s.complete = 1
                     AND ((?3 IS NULL AND s.daily IS NULL AND s.fingerprint = ?1 AND s.mods = ?2) OR (?3 IS NOT NULL AND s.daily = ?3))
                     AND s.id = (SELECT id FROM scores t WHERE t.player = s.player AND t.complete = 1
                                 AND ((?3 IS NULL AND t.daily IS NULL AND t.fingerprint = ?1 AND t.mods = ?2) OR (?3 IS NOT NULL AND t.daily = ?3))
                                 ORDER BY t.clean DESC, t.score DESC, t.id ASC LIMIT 1)
                   ORDER BY s.clean DESC, s.score DESC, s.id ASC LIMIT ?4";
        let mut st = self.conn.prepare(sql)?;
        let rows = st.query_map(params![fp, mods, daily, limit], |r| {
            Ok(Entry {
                rank: 0,
                id: r.get(0)?,
                player: r.get(1)?,
                name: r.get(2)?,
                score: r.get(3)?,
                pp: r.get(4)?,
                grade: r.get(5)?,
                fc: r.get::<_, i64>(6)? != 0,
                pfc: r.get::<_, i64>(7)? != 0,
                max_combo: r.get(8)?,
                beat_accuracy: r.get(9)?,
                timing_std_ms: r.get(10)?,
                downs: r.get(11)?,
                clean: r.get::<_, i64>(12)? != 0,
                flags: serde_json::from_str(&r.get::<_, String>(13)?).unwrap_or_default(),
                when: r.get(14)?,
            })
        })?;
        let mut out = Vec::new();
        for (i, e) in rows.enumerate() {
            let mut e = e?;
            e.rank = i as u32 + 1;
            out.push(e);
        }
        Ok(out)
    }

    /// Rank of a score id on its board (1-based), by the board's order.
    pub fn rank_of(&self, id: i64) -> rusqlite::Result<Option<u32>> {
        let row: Option<(String, String, Option<String>)> =
            self.conn.query_row("SELECT fingerprint, mods, daily FROM scores WHERE id = ?1", params![id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional()?;
        let Some((fp, mods, daily)) = row else { return Ok(None) };
        let fingerprint = u64::from_str_radix(&fp, 16).ok();
        let board = self.board(fingerprint, &mods, daily.as_deref(), 100_000)?;
        Ok(board.iter().find(|e| e.id == id).map(|e| e.rank))
    }

    pub fn replay(&self, id: i64) -> rusqlite::Result<Option<String>> {
        self.conn.query_row("SELECT replay FROM scores WHERE id = ?1", params![id], |r| r.get(0)).optional()
    }

    /// Best clean ranked pp per chart for a player, for the profile rating.
    pub fn best_pp_per_chart(&self, player: &str) -> rusqlite::Result<Vec<(String, f64)>> {
        let mut st = self.conn.prepare("SELECT fingerprint, MAX(pp) FROM scores WHERE player = ?1 AND clean = 1 AND ranked = 1 AND complete = 1 AND daily IS NULL GROUP BY fingerprint")?;
        let rows = st.query_map(params![player], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.collect()
    }

    pub fn player_plays(&self, player: &str) -> rusqlite::Result<i64> {
        self.conn.query_row("SELECT COUNT(*) FROM scores WHERE player = ?1", params![player], |r| r.get(0))
    }

    pub fn score_count(&self) -> rusqlite::Result<i64> {
        self.conn.query_row("SELECT COUNT(*) FROM scores", [], |r| r.get(0))
    }
}
