//! The online layer end to end, in process: register, submit bot replays,
//! boards and ranks, flags, tampering, unranked charts, the daily, replay
//! download for ghosts, profile rating.

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use downbeat_core::clock::ClockConfig;
use downbeat_core::judge::Windows;
use downbeat_core::patterns::{street_level, warmup_level};
use downbeat_core::score::Scoring;
use downbeat_core::sim::{run_game, run_level, Bot, SimConfig};
use downbeat_core::Game;
use downbeat_server::{app, AppState};
use serde_json::{json, Value};
use tower::ServiceExt;

fn levels_dir() -> String {
    format!("{}/../godot/levels", env!("CARGO_MANIFEST_DIR"))
}

fn test_app() -> Router {
    app(AppState::open(":memory:", &levels_dir()).unwrap())
}

async fn call(app: &Router, method: &str, path: &str, auth: Option<&str>, body: Option<Value>) -> (StatusCode, Value) {
    let mut req = Request::builder().method(method).uri(path);
    if let Some(a) = auth {
        req = req.header("authorization", format!("Bearer {a}"));
    }
    let req = match body {
        Some(b) => req.header("content-type", "application/json").body(Body::from(b.to_string())).unwrap(),
        None => req.body(Body::empty()).unwrap(),
    };
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let v: Value = if bytes.is_empty() { Value::Null } else { serde_json::from_slice(&bytes).unwrap_or(Value::String(String::from_utf8_lossy(&bytes).into())) };
    (status, v)
}

async fn register(app: &Router, name: &str) -> String {
    let (st, v) = call(app, "POST", "/v1/players", None, Some(json!({"name": name}))).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    format!("{}:{}", v["id"].as_str().unwrap(), v["token"].as_str().unwrap())
}

fn street_replay(bot: Bot, seed: u64) -> (String, downbeat_core::score::Report) {
    let r = run_level(&street_level(128.0), bot, SimConfig { seed, ..SimConfig::default() });
    (r.replay.unwrap().to_json(), r.report)
}

#[tokio::test]
async fn health_lists_the_ranked_charts() {
    let app = test_app();
    let (st, v) = call(&app, "GET", "/v1/health", None, None).await;
    assert_eq!(st, StatusCode::OK);
    assert!(v["ok"].as_bool().unwrap());
    // 161 tempos × 4 builtins × 6 mod sets + the three shipped levels × 6, minus
    // the DT/HT charts that coincide with a plain chart at another tempo
    // (the warmup at 100 DT is the warmup at 150).
    assert!(v["ranked_charts"].as_u64().unwrap() >= 3000, "{v}");
}

#[tokio::test]
async fn scores_are_rescored_ranked_and_boarded() {
    let app = test_app();
    let alice = register(&app, "alice").await;
    let bob = register(&app, "bob").await;
    let (ra, rep_a) = street_replay(Bot::human(22.0), 7);
    let (rb, rep_b) = street_replay(Bot::human(40.0), 8);
    assert!(rep_a.score != rep_b.score);
    let (st, va) = call(&app, "POST", "/v1/scores", Some(&alice), Some(json!({"replay": ra}))).await;
    assert_eq!(st, StatusCode::OK, "{va}");
    assert_eq!(va["score"].as_u64().unwrap(), rep_a.score, "the server re-scores to the client's number");
    assert_eq!(va["ranked"], true);
    assert_eq!(va["complete"], true);
    assert_eq!(va["clean"], true);
    assert_eq!(va["rank"], 1);
    let (st, vb) = call(&app, "POST", "/v1/scores", Some(&bob), Some(json!({"replay": rb}))).await;
    assert_eq!(st, StatusCode::OK, "{vb}");
    let (better, worse) = if rep_a.score > rep_b.score { (&va, &vb) } else { (&vb, &va) };
    assert_eq!(worse["rank"], 2);
    assert_eq!(better["rank"], 1);
    let fp = va["fingerprint"].as_str().unwrap();
    let (st, board) = call(&app, "GET", &format!("/v1/boards/{fp}?mods=NM"), None, None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(board["ranked"], true);
    assert_eq!(board["chart"]["name"], "street_128");
    let entries = board["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0]["rank"], 1);
    assert!(entries[0]["score"].as_u64().unwrap() >= entries[1]["score"].as_u64().unwrap());
    // A second, worse score from alice does not demote her board entry; a better one replaces it.
    let (rc, rep_c) = street_replay(Bot::human(60.0), 9);
    assert!(rep_c.score < rep_a.score);
    call(&app, "POST", "/v1/scores", Some(&alice), Some(json!({"replay": rc}))).await;
    let (_, board2) = call(&app, "GET", &format!("/v1/boards/{fp}"), None, None).await;
    let entries2 = board2["entries"].as_array().unwrap();
    assert_eq!(entries2.len(), 2, "one entry per player");
    let alice_entry = entries2.iter().find(|e| e["name"] == "alice").unwrap();
    assert_eq!(alice_entry["score"].as_u64().unwrap(), rep_a.score);
    // Profile rating counts the best clean ranked pp per chart.
    let alice_id = alice.split(':').next().unwrap();
    let (st, prof) = call(&app, "GET", &format!("/v1/players/{alice_id}"), None, None).await;
    assert_eq!(st, StatusCode::OK);
    assert!(prof["rating"].as_f64().unwrap() > 0.0);
    assert_eq!(prof["plays"], 2);
    assert_eq!(prof["charts"], 1);
}

#[tokio::test]
async fn a_perfect_bot_is_flagged_and_ranked_below_clean_scores() {
    let app = test_app();
    let cheat = register(&app, "cheat").await;
    let human = register(&app, "human").await;
    let (rh, _) = street_replay(Bot::human(22.0), 7);
    let (rp, rep_p) = street_replay(Bot::perfect(), 1);
    let (_, vh) = call(&app, "POST", "/v1/scores", Some(&human), Some(json!({"replay": rh}))).await;
    let (st, vp) = call(&app, "POST", "/v1/scores", Some(&cheat), Some(json!({"replay": rp}))).await;
    assert_eq!(st, StatusCode::OK, "{vp}");
    assert_eq!(vp["clean"], false);
    assert!(!vp["flags"].as_array().unwrap().is_empty());
    assert_eq!(vp["score"].as_u64().unwrap(), rep_p.score);
    // Higher score, lower rank: flagged scores sit under every clean one.
    assert!(vp["score"].as_u64() > vh["score"].as_u64());
    assert_eq!(vp["rank"], 2);
    let cheat_id = cheat.split(':').next().unwrap();
    let (_, prof) = call(&app, "GET", &format!("/v1/players/{cheat_id}"), None, None).await;
    assert_eq!(prof["rating"].as_f64().unwrap(), 0.0, "flagged scores earn no rating");
}

#[tokio::test]
async fn tampered_and_custom_replays() {
    let app = test_app();
    let p = register(&app, "p").await;
    // Tamper: the level's BPM lowered after the run. The replay's fingerprint
    // no longer matches the chart it describes.
    let (text, _) = street_replay(Bot::human(22.0), 7);
    let mut v: Value = serde_json::from_str(&text).unwrap();
    v["level"]["bpm"] = json!(100.0);
    let (st, e) = call(&app, "POST", "/v1/scores", Some(&p), Some(json!({"replay": v}))).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{e}");
    assert!(e["error"].as_str().unwrap().contains("chart changed"), "{e}");
    // Garbage.
    let (st, _) = call(&app, "POST", "/v1/scores", Some(&p), Some(json!({"replay": "{not json"}))).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    // A custom chart (the warmup with one more phrase): accepted, re-scored,
    // on its own board, unranked.
    let mut custom = warmup_level(128.0);
    custom.sections.push(downbeat_core::chart::Section { pattern: "flick_8ths".into(), repeats: 2 });
    let r = run_level(&custom, Bot::human(22.0), SimConfig::default());
    let (st, v) = call(&app, "POST", "/v1/scores", Some(&p), Some(json!({"replay": r.replay.unwrap().to_json()}))).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(v["ranked"], false);
    assert_eq!(v["rank"], 1);
    let fp = v["fingerprint"].as_str().unwrap();
    let (_, board) = call(&app, "GET", &format!("/v1/boards/{fp}"), None, None).await;
    assert_eq!(board["ranked"], false);
    assert_eq!(board["entries"].as_array().unwrap().len(), 1);
    // No token, bad token.
    let (st, _) = call(&app, "POST", "/v1/scores", None, Some(json!({"replay": text}))).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
    let (st, _) = call(&app, "POST", "/v1/scores", Some("nobody:nothing"), Some(json!({"replay": text}))).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn an_abandoned_run_is_kept_but_not_ranked() {
    let app = test_app();
    let p = register(&app, "p").await;
    let (text, _) = street_replay(Bot::human(22.0), 7);
    let mut v: Value = serde_json::from_str(&text).unwrap();
    // Half the run: the replay stops at a chart time before the end.
    v["end_chart_time"] = json!(20.0);
    let inputs: Vec<Value> = v["inputs"].as_array().unwrap().iter().filter(|i| i["at"].as_f64().unwrap() <= 20.0).cloned().collect();
    v["inputs"] = Value::Array(inputs);
    let (st, out) = call(&app, "POST", "/v1/scores", Some(&p), Some(json!({"replay": v}))).await;
    assert_eq!(st, StatusCode::OK, "{out}");
    assert_eq!(out["complete"], false);
    assert!(out["rank"].is_null());
    let fp = out["fingerprint"].as_str().unwrap();
    let (_, board) = call(&app, "GET", &format!("/v1/boards/{fp}"), None, None).await;
    assert_eq!(board["entries"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn replays_download_for_ghosts() {
    let app = test_app();
    let p = register(&app, "p").await;
    let (text, rep) = street_replay(Bot::human(22.0), 7);
    let (_, v) = call(&app, "POST", "/v1/scores", Some(&p), Some(json!({"replay": text}))).await;
    let id = v["id"].as_i64().unwrap();
    let (st, got) = call(&app, "GET", &format!("/v1/replays/{id}"), None, None).await;
    assert_eq!(st, StatusCode::OK);
    let replay = downbeat_core::replay::Replay::from_json(&got.to_string()).unwrap();
    assert_eq!(replay.play().unwrap(), rep, "the stored replay re-scores to the same report");
    let (st, _) = call(&app, "GET", "/v1/replays/999", None, None).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_daily_is_served_played_and_boarded() {
    let app = test_app();
    let p = register(&app, "p").await;
    let (st, d) = call(&app, "GET", "/v1/daily", None, None).await;
    assert_eq!(st, StatusCode::OK, "{d}");
    let level: downbeat_core::chart::Level = serde_json::from_value(d["level"].clone()).unwrap();
    let mods = downbeat_server::mods_from_label(d["mods"].as_str().unwrap());
    let game = Game::from_level_with_mods(&level, mods, ClockConfig::default(), Windows::default(), Scoring::default()).unwrap();
    assert_eq!(format!("{:016x}", game.chart().fingerprint()), d["fingerprint"].as_str().unwrap());
    let r = run_game(game, Bot::human(22.0), SimConfig { seed: 5, ..SimConfig::default() });
    let text = r.replay.unwrap().to_json();
    let (st, v) = call(&app, "POST", "/v1/daily/scores", Some(&p), Some(json!({"replay": text}))).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(v["daily"].as_str().unwrap(), d["date"].as_str().unwrap());
    assert_eq!(v["rank"], 1);
    let (st, board) = call(&app, "GET", "/v1/daily/board", None, None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(board["entries"].as_array().unwrap().len(), 1);
    assert_eq!(board["title"], d["title"]);
    // A replay on some other chart is not a daily score.
    let (other, _) = street_replay(Bot::human(22.0), 7);
    let (st, e) = call(&app, "POST", "/v1/daily/scores", Some(&p), Some(json!({"replay": other}))).await;
    assert!(st == StatusCode::UNPROCESSABLE_ENTITY || v["fingerprint"] == e["fingerprint"], "{e}");
    // Any date resolves, deterministically.
    let (_, a) = call(&app, "GET", "/v1/daily?date=2026-11-01", None, None).await;
    let (_, b) = call(&app, "GET", "/v1/daily?date=2026-11-01", None, None).await;
    assert_eq!(a["fingerprint"], b["fingerprint"]);
    let (st, _) = call(&app, "GET", "/v1/daily?date=nope", None, None).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn shipped_levels_are_served() {
    let app = test_app();
    let (st, v) = call(&app, "GET", "/v1/levels/skyline_168", None, None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(v["bpm"], 168.0);
    let (st, _) = call(&app, "GET", "/v1/levels/nope", None, None).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
}
