//! dbserver: DOWNBEAT's online layer in one binary.
//!   dbserver --bind 0.0.0.0:8787 --db downbeat.db --levels ../godot/levels
//! Every score is a replay; the server re-scores it with the same core the
//! game runs, inspects it, and ranks it. Put it behind a TLS reverse proxy.

use downbeat_server::{app, AppState};
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut bind = "127.0.0.1:8787".to_string();
    let mut db = "downbeat.db".to_string();
    let mut levels = "../godot/levels".to_string();
    let mut i = 0;
    while i < argv.len() {
        match argv[i].as_str() {
            "--bind" => {
                i += 1;
                bind = argv.get(i).cloned().unwrap_or(bind);
            }
            "--db" => {
                i += 1;
                db = argv.get(i).cloned().unwrap_or(db);
            }
            "--levels" => {
                i += 1;
                levels = argv.get(i).cloned().unwrap_or(levels);
            }
            "-h" | "--help" => {
                println!("dbserver [--bind ADDR] [--db FILE] [--levels DIR]");
                return;
            }
            other => {
                eprintln!("unknown flag {other}");
                std::process::exit(2);
            }
        }
        i += 1;
    }
    let state = match AppState::open(&db, &levels) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("dbserver: {e}");
            std::process::exit(1);
        }
    };
    let charts = state.charts.ranked.len();
    let addr: SocketAddr = bind.parse().expect("bind address");
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind");
    println!("DOWNBEAT server  {addr}  db {db}  {charts} ranked charts");
    axum::serve(listener, app(state))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .expect("serve");
}
