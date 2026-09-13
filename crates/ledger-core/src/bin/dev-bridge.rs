//! Explicit development-only browser bridge. Never included in the native application.
use anyhow::{ensure, Context, Result};
use ledger_core::{now, Store};
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};
fn execute(s: &mut Store, name: &str, args: Value) -> Result<Value> {
    let text = |k: &str| -> Result<String> {
        Ok(args[k].as_str().context(format!("Missing {k}"))?.into())
    };
    match name {
        "snapshot" => {}
        "add_category" => {
            s.add_category(&text("name")?, now())?;
        }
        "set_category_color" => s.set_category_color(&text("category")?, &text("color")?, now())?,
        "set_device_priority" => {
            s.set_device_priority(serde_json::from_value(args["devices"].clone())?, now())?
        }
        "save_entry" => {
            s.save_entry_local(
                args["id"].as_str().map(String::from),
                text("category")?,
                &text("start")?,
                &text("end")?,
                now(),
            )?;
        }
        "delete_entry" => s.delete_entry(&text("id")?, now())?,
        "start_timer" => {
            s.start_timer(text("category")?, now())?;
        }
        "stop_timer" => {
            s.stop_timer(now())?;
        }
        "report" => {
            return Ok(serde_json::to_value(s.report(
                &text("period")?,
                &text("date")?,
                now(),
            )?)?)
        }
        "configure_updates" => s.set_updates(text("url")?, text("key")?)?,
        "sync_tick" => return Ok(Value::Null),
        "sync_status" => {
            return Ok(
                json!({"port":0,"peers":[],"pending":[],"joining_code":null,"last_sync":null,"error":null,"active":true}),
            )
        }
        _ => anyhow::bail!("This command is available in the native application"),
    }
    Ok(serde_json::to_value(s.snapshot())?)
}
fn handle(mut stream: TcpStream, store: Arc<Mutex<Store>>) -> Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut data = vec![];
    let header_end = loop {
        let mut chunk = [0u8; 1024];
        let n = stream.read(&mut chunk)?;
        ensure!(n > 0, "Incomplete request");
        data.extend_from_slice(&chunk[..n]);
        ensure!(data.len() < 65536, "Request too large");
        if let Some(p) = data.windows(4).position(|v| v == b"\r\n\r\n") {
            break p + 4;
        }
    };
    let header = String::from_utf8(data[..header_end].to_vec())?;
    let request: Vec<_> = header
        .lines()
        .next()
        .unwrap_or("")
        .split_whitespace()
        .collect();
    let origin = header.lines().find_map(|l| {
        l.split_once(':')
            .filter(|(k, _)| k.eq_ignore_ascii_case("origin"))
            .map(|(_, v)| v.trim())
    });
    ensure!(
        origin.is_none()
            || matches!(
                origin,
                Some("http://localhost:1420" | "http://127.0.0.1:1420")
            ),
        "Disallowed origin"
    );
    let length: usize = header
        .lines()
        .find_map(|l| {
            l.split_once(':')
                .filter(|(k, _)| k.eq_ignore_ascii_case("content-length"))
                .and_then(|(_, v)| v.trim().parse().ok())
        })
        .unwrap_or(0);
    ensure!(length <= 65536, "Body too large");
    while data.len() - header_end < length {
        let mut chunk = [0u8; 1024];
        let n = stream.read(&mut chunk)?;
        ensure!(n > 0, "Incomplete body");
        data.extend_from_slice(&chunk[..n]);
    }
    let result = if request.first() == Some(&"OPTIONS") {
        json!({})
    } else {
        ensure!(request.first() == Some(&"POST"), "POST required");
        let args = serde_json::from_slice(&data[header_end..header_end + length])?;
        match execute(
            &mut store.lock().unwrap(),
            request.get(1).unwrap_or(&"").trim_start_matches('/'),
            args,
        ) {
            Ok(value) => json!({"value":value}),
            Err(e) => json!({"error":e.to_string()}),
        }
    };
    let body = serde_json::to_vec(&result)?;
    write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: {}\r\nAccess-Control-Allow-Methods: POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",origin.unwrap_or("http://localhost:1420"),body.len())?;
    stream.write_all(&body)?;
    Ok(())
}
fn main() -> Result<()> {
    let path = std::env::args()
        .nth(1)
        .context("Usage: dev-bridge /path/to/development.sqlite3")?;
    let store = Arc::new(Mutex::new(Store::open(
        path,
        "Development preview".into(),
        "Europe/Minsk",
    )?));
    let listener = TcpListener::bind("127.0.0.1:1421")?;
    println!("Development-only Rust bridge on 127.0.0.1:1421");
    for stream in listener.incoming() {
        let s = store.clone();
        thread::spawn(move || {
            if let Ok(stream) = stream {
                let _ = handle(stream, s);
            }
        });
    }
    Ok(())
}
