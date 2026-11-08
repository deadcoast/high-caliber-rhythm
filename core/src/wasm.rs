//! The core for the web chart editor: a C ABI over the same model, built
//! for `wasm32-unknown-unknown` with no bindgen tooling. Every call takes a
//! JSON request and returns a JSON reply; audio comes in as raw f32 samples.
//!
//! Protocol (web/src/core.js is the other half):
//!   db_alloc(len) -> ptr          caller-owned bytes for a request
//!   db_free(ptr, len)
//!   db_call(req_ptr, req_len) -> reply_ptr; db_reply_len() -> reply_len
//!       the reply buffer lives until the next call
//!   db_analyze(samples_ptr, frames, sample_rate) -> reply as above
//!
//! Ops (`{"op": ..., ...}`):
//!   patterns            builtin pattern library
//!   rooms               builtin rooms
//!   drills              Range drills
//!   compile {level}     chart: targets, rooms, spans, fingerprint, rating
//!   rate {level}        rating only
//!   simulate {level, bot?, seed?}   a bot plays it: report + restarts
//!   auto_level {name, audio, bpm, offset_s, duration_s, energy}
//!   regrid {bpm, offset_s}          energy per bar + onset grid of the last analysed song
//!   version

use crate::analysis::{analyze, auto_level, bar_energy, Analysis};
use crate::chart::{compile, Level};
use crate::patterns::{builtin_lib, builtin_patterns, builtin_rooms, drills};
use crate::rating::rate;
use crate::sim::{run_level, Bot, SimConfig};
use serde_json::{json, Value};
use std::cell::RefCell;

thread_local! {
    static REPLY: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static LAST_ANALYSIS: RefCell<Option<Analysis>> = const { RefCell::new(None) };
}

fn set_reply(v: Value) -> *const u8 {
    let bytes = serde_json::to_vec(&v).unwrap_or_default();
    REPLY.with(|r| {
        *r.borrow_mut() = bytes;
        r.borrow().as_ptr()
    })
}

#[no_mangle]
pub extern "C" fn db_alloc(len: usize) -> *mut u8 {
    let mut v = Vec::<u8>::with_capacity(len.max(1));
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

/// # Safety
/// `ptr` must come from `db_alloc(len)` and not have been freed.
#[no_mangle]
pub unsafe extern "C" fn db_free(ptr: *mut u8, len: usize) {
    if !ptr.is_null() {
        drop(Vec::from_raw_parts(ptr, 0, len.max(1)));
    }
}

#[no_mangle]
pub extern "C" fn db_reply_len() -> usize {
    REPLY.with(|r| r.borrow().len())
}

/// # Safety
/// `ptr..ptr+len` must be a readable UTF-8 JSON request.
#[no_mangle]
pub unsafe extern "C" fn db_call(ptr: *const u8, len: usize) -> *const u8 {
    let bytes = std::slice::from_raw_parts(ptr, len);
    let req: Value = match serde_json::from_slice(bytes) {
        Ok(v) => v,
        Err(e) => return set_reply(json!({"error": format!("request: {e}")})),
    };
    set_reply(dispatch(&req))
}

/// # Safety
/// `ptr` must point at `frames` readable f32 samples (mono).
#[no_mangle]
pub unsafe extern "C" fn db_analyze(ptr: *const f32, frames: usize, sample_rate: u32) -> *const u8 {
    let samples = std::slice::from_raw_parts(ptr, frames);
    let a = analyze(samples, sample_rate);
    let reply = analysis_value(&a, a.bpm, a.offset_s);
    LAST_ANALYSIS.with(|l| *l.borrow_mut() = Some(a));
    set_reply(reply)
}

fn analysis_value(a: &Analysis, bpm: f64, offset_s: f64) -> Value {
    let bars = (((a.duration_s - offset_s) / (60.0 / bpm.max(1.0))) / 4.0).floor().max(0.0) as usize;
    let energy = bar_energy(a, bpm, offset_s, bars);
    // Onset strength downsampled to 20 per second for the editor's timeline.
    let step = (a.frame_rate / 20.0).max(1.0);
    let mut onset = Vec::new();
    let mut i = 0.0;
    while (i as usize) < a.onset.len() {
        let end = ((i + step) as usize).min(a.onset.len());
        let seg = &a.onset[i as usize..end];
        onset.push(seg.iter().cloned().fold(0.0f32, f32::max));
        i += step;
    }
    let peak = onset.iter().cloned().fold(0.0f32, f32::max).max(1e-6);
    let onset: Vec<f32> = onset.iter().map(|v| v / peak).collect();
    json!({
        "bpm": bpm,
        "offset_s": offset_s,
        "duration_s": a.duration_s,
        "tempo_confidence": a.tempo_confidence,
        "phase_confidence": a.phase_confidence,
        "bars": bars,
        "energy": energy,
        "onset_rate": 20.0,
        "onset": onset,
    })
}

fn level_from(req: &Value) -> Result<Level, String> {
    let lv = req.get("level").ok_or("missing level")?;
    serde_json::from_value::<Level>(lv.clone()).map_err(|e| format!("level: {e}"))
}

fn dispatch(req: &Value) -> Value {
    let op = req.get("op").and_then(Value::as_str).unwrap_or("");
    match op {
        "version" => json!({"version": env!("CARGO_PKG_VERSION"), "ops": ["patterns", "rooms", "drills", "compile", "rate", "simulate", "auto_level", "regrid", "version"]}),
        "patterns" => json!({"patterns": builtin_patterns()}),
        "rooms" => json!({"rooms": builtin_rooms()}),
        "drills" => json!({"drills": drills()}),
        "compile" => match level_from(req) {
            Ok(level) => match compile(&level, &builtin_lib()) {
                Ok(chart) => {
                    let rating = rate(&chart);
                    json!({
                        "fingerprint": format!("{:016x}", chart.fingerprint()),
                        "bpm": chart.bpm,
                        "total_beats": chart.total_beats,
                        "total_time": chart.total_time,
                        "targets": chart.targets,
                        "rooms": chart.rooms,
                        "spans": chart.spans,
                        "phases": chart.phases,
                        "cues": chart.cues,
                        "rating": rating,
                    })
                }
                Err(e) => json!({"error": format!("{e}")}),
            },
            Err(e) => json!({"error": e}),
        },
        "rate" => match level_from(req) {
            Ok(level) => match compile(&level, &builtin_lib()) {
                Ok(chart) => json!({"rating": rate(&chart)}),
                Err(e) => json!({"error": format!("{e}")}),
            },
            Err(e) => json!({"error": e}),
        },
        "simulate" => match level_from(req) {
            Ok(level) => {
                let mut bot = Bot::human(22.0);
                if let Some(b) = req.get("bot") {
                    if let Some(v) = b.get("jitter_ms").and_then(Value::as_f64) {
                        bot.jitter_ms = v;
                    }
                    if let Some(v) = b.get("bias_ms").and_then(Value::as_f64) {
                        bot.bias_ms = v;
                    }
                    if let Some(v) = b.get("hit_prob").and_then(Value::as_f64) {
                        bot.hit_prob = v;
                    }
                    if let Some(v) = b.get("skip_prob").and_then(Value::as_f64) {
                        bot.skip_prob = v;
                    }
                    if b.get("perfect").and_then(Value::as_bool) == Some(true) {
                        bot = Bot::perfect();
                    }
                }
                let seed = req.get("seed").and_then(Value::as_u64).unwrap_or(7);
                let r = run_level(&level, bot, SimConfig { seed, audio_clock: false, ..SimConfig::default() });
                json!({"report": r.report, "restarts": r.restarts, "spawns": r.spawns, "sim_time_s": r.sim_time_s})
            }
            Err(e) => json!({"error": e}),
        },
        "auto_level" => {
            let name = req.get("name").and_then(Value::as_str).unwrap_or("song");
            let audio = req.get("audio").and_then(Value::as_str).unwrap_or("");
            let bpm = req.get("bpm").and_then(Value::as_f64).unwrap_or(128.0);
            let offset = req.get("offset_s").and_then(Value::as_f64).unwrap_or(0.0);
            let duration = req.get("duration_s").and_then(Value::as_f64).unwrap_or(0.0);
            let energy: Vec<f64> = req
                .get("energy")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_f64).collect())
                .unwrap_or_default();
            json!({"level": auto_level(name, audio, bpm, offset, duration, &energy)})
        }
        "regrid" => {
            let bpm = req.get("bpm").and_then(Value::as_f64).unwrap_or(128.0);
            let offset = req.get("offset_s").and_then(Value::as_f64).unwrap_or(0.0);
            LAST_ANALYSIS.with(|l| match l.borrow().as_ref() {
                Some(a) => analysis_value(a, bpm, offset),
                None => json!({"error": "no song analysed yet"}),
            })
        }
        other => json!({"error": format!("unknown op '{other}'")}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_compiles_rates_and_simulates_the_warmup() {
        let level = crate::patterns::warmup_level(128.0);
        let req = json!({"op": "compile", "level": level});
        let out = dispatch(&req);
        assert!(out.get("error").is_none(), "{out}");
        assert_eq!(out["targets"].as_array().unwrap().len(), 70);
        assert!((out["rating"]["stars"].as_f64().unwrap() - 3.0).abs() < 0.02);
        let sim = dispatch(&json!({"op": "simulate", "level": level, "bot": {"perfect": true}}));
        assert_eq!(sim["report"]["expired"].as_u64().unwrap(), 0);
        assert!(dispatch(&json!({"op": "patterns"}))["patterns"].as_array().unwrap().len() >= 10);
        assert!(dispatch(&json!({"op": "nope"}))["error"].is_string());
        assert!(dispatch(&json!({"op": "compile", "level": {"bpm": "x"}}))["error"].is_string());
    }

    #[test]
    fn the_c_abi_round_trips_a_request() {
        let req = br#"{"op":"version"}"#;
        let p = db_alloc(req.len());
        unsafe {
            std::ptr::copy_nonoverlapping(req.as_ptr(), p, req.len());
            let out = db_call(p, req.len());
            let len = db_reply_len();
            let s = std::str::from_utf8(std::slice::from_raw_parts(out, len)).unwrap();
            assert!(s.contains("\"ops\""), "{s}");
            db_free(p, req.len());
        }
        let sr = 44100u32;
        let samples = crate::analysis::synth_track(128.0, 0.3, 24.0, sr, 3);
        unsafe {
            let out = db_analyze(samples.as_ptr(), samples.len(), sr);
            let len = db_reply_len();
            let v: Value = serde_json::from_slice(std::slice::from_raw_parts(out, len)).unwrap();
            assert!((v["bpm"].as_f64().unwrap() - 128.0).abs() < 0.5, "{v}");
            assert_eq!(v["onset_rate"].as_f64().unwrap(), 20.0);
            assert!(v["energy"].as_array().unwrap().len() >= 10);
        }
        let re = dispatch(&json!({"op": "regrid", "bpm": 120.0, "offset_s": 0.0}));
        assert!(re.get("energy").is_some(), "{re}");
    }
}
