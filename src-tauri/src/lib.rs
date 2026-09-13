#[cfg(not(target_os = "android"))]
mod desktop_updates;
use ledger_core::{
    now,
    sync::{Network, Peer, Status},
    updates::{self, Manifest},
    Report, Snapshot, Store,
};
use std::sync::{Arc, Mutex};
use tauri::{Manager, State};
struct AppState {
    store: Arc<Mutex<Store>>,
    network: Option<Arc<Network>>,
    network_error: Option<String>,
}
type CommandResult<T> = Result<T, String>;
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
#[tauri::command]
fn snapshot(state: State<AppState>) -> CommandResult<Snapshot> {
    Ok(state.store.lock().map_err(err)?.snapshot())
}
#[tauri::command]
fn add_category(name: String, state: State<AppState>) -> CommandResult<Snapshot> {
    let mut s = state.store.lock().map_err(err)?;
    s.add_category(&name, now()).map_err(err)?;
    Ok(s.snapshot())
}
#[tauri::command]
fn save_entry(
    id: Option<String>,
    category: String,
    start: String,
    end: String,
    state: State<AppState>,
) -> CommandResult<Snapshot> {
    let mut s = state.store.lock().map_err(err)?;
    s.save_entry_local(id, category, &start, &end, now())
        .map_err(err)?;
    Ok(s.snapshot())
}
#[tauri::command]
fn delete_entry(id: String, state: State<AppState>) -> CommandResult<Snapshot> {
    let mut s = state.store.lock().map_err(err)?;
    s.delete_entry(&id, now()).map_err(err)?;
    Ok(s.snapshot())
}
#[tauri::command]
fn start_timer(category: String, state: State<AppState>) -> CommandResult<Snapshot> {
    let mut s = state.store.lock().map_err(err)?;
    s.start_timer(category, now()).map_err(err)?;
    Ok(s.snapshot())
}
#[tauri::command]
fn stop_timer(state: State<AppState>) -> CommandResult<Snapshot> {
    let mut s = state.store.lock().map_err(err)?;
    s.stop_timer(now()).map_err(err)?;
    Ok(s.snapshot())
}
#[tauri::command]
fn report(period: String, date: String, state: State<AppState>) -> CommandResult<Report> {
    state
        .store
        .lock()
        .map_err(err)?
        .report(&period, &date, now())
        .map_err(err)
}
#[tauri::command]
fn sync_status(state: State<AppState>) -> CommandResult<Status> {
    state.network.as_ref().map(|n| n.status()).ok_or_else(|| {
        state
            .network_error
            .clone()
            .unwrap_or("Network unavailable".into())
    })
}
#[tauri::command]
fn sync_tick(active: bool, state: State<AppState>) {
    if let Some(n) = &state.network {
        n.set_active(active);
        if active {
            n.tick();
        }
    }
}
#[tauri::command]
fn nearby_peer(key: String, peer: Option<Peer>, state: State<AppState>) {
    if let Some(n) = &state.network {
        n.peer(key, peer);
    }
}
#[tauri::command]
fn join_peer(address: String, state: State<AppState>) -> CommandResult<()> {
    state
        .network
        .as_ref()
        .ok_or("Network unavailable")?
        .join_peer(address)
        .map_err(err)
}
#[tauri::command]
fn approve_join(id: String, accept: bool, state: State<AppState>) -> CommandResult<()> {
    state
        .network
        .as_ref()
        .ok_or("Network unavailable")?
        .approve(&id, accept)
        .map_err(err)
}
#[tauri::command]
fn configure_updates(url: String, key: String, state: State<AppState>) -> CommandResult<Snapshot> {
    let mut s = state.store.lock().map_err(err)?;
    s.set_updates(url, key).map_err(err)?;
    Ok(s.snapshot())
}
// Defaults are bundled with the app; custom sources remain installation-local.
fn update_config(state: &AppState) -> CommandResult<ledger_core::Config> {
    let mut config = state.store.lock().map_err(err)?.snapshot().config;
    if config.release_url.is_empty() {
        let defaults: serde_json::Value =
            serde_json::from_str(include_str!("../../release-config.json")).map_err(err)?;
        config.release_url = defaults["android_url"]
            .as_str()
            .ok_or("Missing release URL")?
            .into();
        config.release_key = defaults["android_public_key"]
            .as_str()
            .ok_or("Missing verification key")?
            .into();
    }
    Ok(config)
}
#[tauri::command]
fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
#[tauri::command]
async fn check_update(state: State<'_, AppState>) -> CommandResult<Option<Manifest>> {
    let config = update_config(&state)?;

    state
        .store
        .lock()
        .map_err(err)?
        .mark_update_check(now())
        .map_err(err)?;
    tauri::async_runtime::spawn_blocking(move || {
        updates::check(
            &config.release_url,
            &config.release_key,
            env!("CARGO_PKG_VERSION"),
        )
        .map_err(err)
    })
    .await
    .map_err(err)?
}
#[tauri::command]
async fn download_update(
    manifest: Manifest,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<String> {
    let config = update_config(&state)?;
    let directory = app.path().app_cache_dir().map_err(err)?.join("updates");
    tauri::async_runtime::spawn_blocking(move || {
        updates::download(
            &manifest,
            &config.release_key,
            env!("CARGO_PKG_VERSION"),
            &directory,
        )
        .map(|p| p.to_string_lossy().to_string())
        .map_err(err)
    })
    .await
    .map_err(err)?
}
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(not(target_os = "android"))]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _, _| {
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.show();
            let _ = w.set_focus();
        }
    }));
    #[cfg(not(target_os = "android"))]
    let builder = builder
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(desktop_updates::PendingUpdate::default());
    builder
        .plugin(tauri_plugin_nearby::init())
        .setup(|app| {
            let path = app.path().app_data_dir()?;
            std::fs::create_dir_all(&path)?;
            let name = if cfg!(target_os = "android") {
                String::from("Android phone")
            } else {
                std::env::var("HOSTNAME")
                    .ok()
                    .filter(|s| !s.is_empty())
                    .or_else(|| {
                        std::fs::read_to_string("/etc/hostname")
                            .ok()
                            .map(|s| s.trim().into())
                    })
                    .unwrap_or("Linux computer".into())
            };
            let timezone = iana_time_zone::get_timezone().unwrap_or("UTC".into());
            let store = Arc::new(Mutex::new(Store::open(
                path.join("ledger.sqlite3"),
                name,
                &timezone,
            )?));
            let (network, network_error) = match Network::start(store.clone()) {
                Ok(n) => (Some(n), None),
                Err(e) => (None, Some(e.to_string())),
            };
            app.manage(AppState {
                store,
                network,
                network_error,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_version,
            #[cfg(not(target_os = "android"))]
            desktop_updates::check_desktop_update,
            #[cfg(not(target_os = "android"))]
            desktop_updates::install_desktop_update,
            snapshot,
            add_category,
            save_entry,
            delete_entry,
            start_timer,
            stop_timer,
            report,
            sync_status,
            sync_tick,
            nearby_peer,
            join_peer,
            approve_join,
            configure_updates,
            check_update,
            download_update
        ])
        .run(tauri::generate_context!())
        .expect("Could not start Time Ledger");
}
