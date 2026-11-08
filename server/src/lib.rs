//! DOWNBEAT online. Every score is a replay. The server:
//!   1. parses the replay and re-scores it with the same core the game runs
//!      (the chart fingerprint has to match the level the replay carries);
//!   2. inspects it (`integrity::inspect`): a flagged score is kept, shown
//!      with its flags, ranked below every clean one and left out of ratings;
//!   3. ranks it when the chart is one the server computed itself (shipped
//!      levels, the builtins at any whole BPM), otherwise keeps it on that
//!      chart's own unranked board;
//!   4. serves boards, replays (for ghosts), the daily and profile ratings.
//! Players register once and get a token; the token keeps a name attached
//! to its scores, nothing more. The re-scoring is the anti-cheat.

pub mod charts;
pub mod daily;
pub mod store;

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use downbeat_core::integrity;
use downbeat_core::replay::Replay;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use std::sync::{Arc, Mutex};

use charts::Charts;
use store::{NewScore, Store};

pub const MAX_BODY: usize = 16 * 1024 * 1024;
pub const BOARD_LIMIT: u32 = 50;

pub struct AppState {
    pub store: Mutex<Store>,
    pub charts: Charts,
}

impl AppState {
    pub fn open(db: &str, level_dir: &str) -> Result<Arc<AppState>, String> {
        let store = Store::open(db).map_err(|e| format!("db {db}: {e}"))?;
        Ok(Arc::new(AppState { store: Mutex::new(store), charts: Charts::build(level_dir) }))
    }
}

type App = Arc<AppState>;

pub fn app(state: App) -> Router {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/players", post(register))
        .route("/v1/players/{id}", get(player))
        .route("/v1/scores", post(submit_score))
        .route("/v1/boards/{fingerprint}", get(board))
        .route("/v1/replays/{id}", get(replay))
        .route("/v1/daily", get(daily))
        .route("/v1/daily/scores", post(submit_daily))
        .route("/v1/daily/board", get(daily_board))
        .route("/v1/levels/{name}", get(level))
        .layer(DefaultBodyLimit::max(MAX_BODY))
        .with_state(state)
}

#[derive(Debug)]
pub struct ApiError(StatusCode, String);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"error": self.1}))).into_response()
    }
}

fn bad(msg: impl Into<String>) -> ApiError {
    ApiError(StatusCode::UNPROCESSABLE_ENTITY, msg.into())
}

fn internal(e: impl std::fmt::Display) -> ApiError {
    ApiError(StatusCode::INTERNAL_SERVER_ERROR, format!("{e}"))
}

fn hash_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

async fn health(State(s): State<App>) -> Result<Json<Value>, ApiError> {
    let n = s.store.lock().map_err(internal)?.score_count().map_err(internal)?;
    Ok(Json(json!({"ok": true, "ranked_charts": s.charts.ranked.len(), "scores": n, "today": daily::today_utc()})))
}

#[derive(Deserialize)]
pub struct RegisterBody {
    pub name: String,
}

async fn register(State(s): State<App>, Json(body): Json<RegisterBody>) -> Result<Json<Value>, ApiError> {
    let name = body.name.trim();
    if name.is_empty() || name.chars().count() > 24 {
        return Err(bad("name: 1 to 24 characters"));
    }
    let id = uuid::Uuid::new_v4().to_string();
    let token = hex::encode(uuid::Uuid::new_v4().as_bytes()) + &hex::encode(uuid::Uuid::new_v4().as_bytes());
    s.store.lock().map_err(internal)?.create_player(&id, &hash_token(&token), name).map_err(internal)?;
    Ok(Json(json!({"id": id, "token": token, "name": name})))
}

/// `Authorization: Bearer <id>:<token>`.
fn auth(s: &AppState, headers: &HeaderMap) -> Result<String, ApiError> {
    let h = headers.get("authorization").and_then(|v| v.to_str().ok()).unwrap_or("");
    let Some(rest) = h.strip_prefix("Bearer ") else { return Err(ApiError(StatusCode::UNAUTHORIZED, "bearer token required".into())) };
    let Some((id, token)) = rest.split_once(':') else { return Err(ApiError(StatusCode::UNAUTHORIZED, "token is id:secret".into())) };
    let store = s.store.lock().map_err(internal)?;
    match store.player_token_hash(id).map_err(internal)? {
        Some(h) if h == hash_token(token) => Ok(id.to_string()),
        _ => Err(ApiError(StatusCode::UNAUTHORIZED, "unknown player or bad token".into())),
    }
}

/// What a submitted replay is worth, computed the same way for the normal and the daily boards.
pub struct Scored {
    pub replay: Replay,
    pub report: downbeat_core::score::Report,
    pub fingerprint: u64,
    pub clean: bool,
    pub flags: Vec<String>,
    pub complete: bool,
    pub total_time: f64,
}

pub fn score_replay(text: &str) -> Result<Scored, ApiError> {
    if text.len() > MAX_BODY {
        return Err(bad("replay too large"));
    }
    let replay = Replay::from_json(text).map_err(|e| bad(format!("replay json: {e}")))?;
    if replay.inputs.len() > 200_000 {
        return Err(bad("replay has too many inputs"));
    }
    let report = replay.play().map_err(|e| bad(format!("replay: {e}")))?;
    let game = downbeat_core::Game::from_level_with_mods(&replay.level, replay.mods, replay.clock, replay.windows, replay.scoring).map_err(|e| bad(format!("{e}")))?;
    let total_time = game.chart().total_time;
    let complete = replay.end_chart_time + 1e-6 >= total_time;
    let i = integrity::inspect(&report, &replay);
    Ok(Scored { fingerprint: replay.chart_fingerprint, clean: i.clean(), flags: i.flags, complete, total_time, replay, report })
}

#[derive(Deserialize)]
pub struct ScoreBody {
    /// The replay as the game saved it (object or string).
    pub replay: Value,
}

fn replay_text(body: &ScoreBody) -> Result<String, ApiError> {
    match &body.replay {
        Value::String(s) => Ok(s.clone()),
        v @ Value::Object(_) => Ok(v.to_string()),
        _ => Err(bad("replay: object or string")),
    }
}

fn store_scored(s: &AppState, player: &str, scored: &Scored, text: &str, daily: Option<&str>, ranked: bool) -> Result<(i64, Option<u32>), ApiError> {
    let r = &scored.report;
    let store = s.store.lock().map_err(internal)?;
    let id = store
        .insert_score(&NewScore {
            player,
            fingerprint: scored.fingerprint,
            mods: &r.mods,
            level: &scored.replay.level.name,
            bpm: scored.replay.level.bpm,
            score: r.score as i64,
            pp: r.pp,
            grade: r.grade.label(),
            fc: r.fc,
            pfc: r.pfc,
            max_combo: r.max_combo as i64,
            beat_accuracy: r.beat_accuracy,
            timing_std_ms: r.timing_std_ms,
            downs: r.downs as i64,
            clean: scored.clean,
            flags: &scored.flags,
            ranked,
            complete: scored.complete,
            daily,
            replay: text,
        })
        .map_err(internal)?;
    let rank = if scored.complete { store.rank_of(id).map_err(internal)? } else { None };
    Ok((id, rank))
}

fn score_json(id: i64, rank: Option<u32>, scored: &Scored, ranked: bool, daily: Option<&str>) -> Value {
    let r = &scored.report;
    json!({
        "id": id,
        "rank": rank,
        "ranked": ranked,
        "daily": daily,
        "complete": scored.complete,
        "clean": scored.clean,
        "flags": scored.flags,
        "fingerprint": format!("{:016x}", scored.fingerprint),
        "level": scored.replay.level.name,
        "mods": r.mods,
        "score": r.score,
        "pp": r.pp,
        "grade": r.grade.label(),
        "fc": r.fc,
        "pfc": r.pfc,
        "max_combo": r.max_combo,
        "beat_accuracy": r.beat_accuracy,
        "timing_std_ms": r.timing_std_ms,
        "total_time": scored.total_time,
    })
}

async fn submit_score(State(s): State<App>, headers: HeaderMap, body: Bytes) -> Result<Json<Value>, ApiError> {
    let player = auth(&s, &headers)?;
    let parsed: ScoreBody = serde_json::from_slice(&body).map_err(|e| bad(format!("body: {e}")))?;
    let text = replay_text(&parsed)?;
    let scored = tokio::task::spawn_blocking(move || score_replay(&text).map(|sc| (sc, text))).await.map_err(internal)??;
    let (scored, text) = scored;
    let ranked = s.charts.get(scored.fingerprint).is_some();
    let (id, rank) = store_scored(&s, &player, &scored, &text, None, ranked)?;
    Ok(Json(score_json(id, rank, &scored, ranked, None)))
}

#[derive(Deserialize)]
pub struct BoardQuery {
    pub mods: Option<String>,
    pub limit: Option<u32>,
}

async fn board(State(s): State<App>, Path(fingerprint): Path<String>, Query(q): Query<BoardQuery>) -> Result<Json<Value>, ApiError> {
    let fp = u64::from_str_radix(&fingerprint, 16).map_err(|_| bad("fingerprint: 16 hex digits"))?;
    let mods = q.mods.unwrap_or_else(|| "NM".into());
    let limit = q.limit.unwrap_or(BOARD_LIMIT).min(200);
    let entries = s.store.lock().map_err(internal)?.board(Some(fp), &mods, None, limit).map_err(internal)?;
    let meta = s.charts.get(fp);
    Ok(Json(json!({
        "fingerprint": fingerprint,
        "mods": mods,
        "ranked": meta.is_some(),
        "chart": meta.map(|m| json!({"name": m.name, "bpm": m.bpm, "mods": m.mods, "shipped": m.shipped})),
        "entries": entries,
    })))
}

async fn replay(State(s): State<App>, Path(id): Path<i64>) -> Result<Response, ApiError> {
    let text = s.store.lock().map_err(internal)?.replay(id).map_err(internal)?;
    match text {
        Some(t) => Ok(([("content-type", "application/json")], t).into_response()),
        None => Err(ApiError(StatusCode::NOT_FOUND, "no such replay".into())),
    }
}

#[derive(Deserialize)]
pub struct DailyQuery {
    pub date: Option<String>,
}

fn daily_value(d: &daily::Daily) -> Value {
    json!({
        "date": d.date,
        "title": d.title,
        "mods": d.mods.label(),
        "fingerprint": format!("{:016x}", d.fingerprint),
        "level": serde_json::to_value(&d.level).unwrap_or(Value::Null),
    })
}

async fn daily(State(s): State<App>, Query(q): Query<DailyQuery>) -> Result<Json<Value>, ApiError> {
    let date = q.date.unwrap_or_else(daily::today_utc);
    let d = daily::daily_for(&date, &s.charts).ok_or_else(|| bad("date: YYYY-MM-DD"))?;
    Ok(Json(daily_value(&d)))
}

async fn submit_daily(State(s): State<App>, headers: HeaderMap, body: Bytes) -> Result<Json<Value>, ApiError> {
    let player = auth(&s, &headers)?;
    let parsed: ScoreBody = serde_json::from_slice(&body).map_err(|e| bad(format!("body: {e}")))?;
    let text = replay_text(&parsed)?;
    let scored = tokio::task::spawn_blocking(move || score_replay(&text).map(|sc| (sc, text))).await.map_err(internal)??;
    let (scored, text) = scored;
    // Today's daily, or yesterday's for a run that started before midnight UTC.
    let today = daily::today_utc();
    let yesterday = daily::civil_from_days(daily::day_number(&today).unwrap_or(0) - 1);
    let date = [today, yesterday]
        .into_iter()
        .find(|d| daily::daily_for(d, &s.charts).map(|x| x.fingerprint == scored.fingerprint && x.mods.label() == scored.report.mods).unwrap_or(false))
        .ok_or_else(|| bad("this replay is not on the daily chart"))?;
    let (id, rank) = store_scored(&s, &player, &scored, &text, Some(&date), true)?;
    Ok(Json(score_json(id, rank, &scored, true, Some(&date))))
}

async fn daily_board(State(s): State<App>, Query(q): Query<DailyQuery>) -> Result<Json<Value>, ApiError> {
    let date = q.date.unwrap_or_else(daily::today_utc);
    let d = daily::daily_for(&date, &s.charts).ok_or_else(|| bad("date: YYYY-MM-DD"))?;
    let entries = s.store.lock().map_err(internal)?.board(None, "", Some(&date), BOARD_LIMIT).map_err(internal)?;
    let mut v = daily_value(&d);
    v["entries"] = serde_json::to_value(entries).unwrap_or(Value::Null);
    if let Value::Object(m) = &mut v {
        m.remove("level");
    }
    Ok(Json(v))
}

#[derive(Serialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub rating: f64,
    pub plays: i64,
    pub charts: usize,
}

async fn player(State(s): State<App>, Path(id): Path<String>) -> Result<Json<Value>, ApiError> {
    let store = s.store.lock().map_err(internal)?;
    let name = store.player_name(&id).map_err(internal)?.ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "no such player".into()))?;
    let best = store.best_pp_per_chart(&id).map_err(internal)?;
    let pps: Vec<f64> = best.iter().map(|(_, pp)| *pp).collect();
    let plays = store.player_plays(&id).map_err(internal)?;
    Ok(Json(json!({"id": id, "name": name, "rating": downbeat_core::rating::profile_rating(&pps), "plays": plays, "charts": best.len()})))
}

async fn level(State(s): State<App>, Path(name): Path<String>) -> Result<Json<Value>, ApiError> {
    match s.charts.levels.get(&name) {
        Some(l) => Ok(Json(serde_json::to_value(l).map_err(internal)?)),
        None => Err(ApiError(StatusCode::NOT_FOUND, "no such level".into())),
    }
}

/// Mods label → set, for callers that key by label.
pub fn mods_from_label(label: &str) -> downbeat_core::mods::Mods {
    downbeat_core::mods::Mods::from_label(label)
}

