use std::sync::Mutex;
use tauri::{AppHandle, State};
use tauri_plugin_updater::{Update, UpdaterExt};

#[derive(Default)]
pub struct PendingUpdate(Mutex<Option<Update>>);

#[derive(serde::Serialize)]
pub struct AvailableUpdate {
    version: String,
}

#[tauri::command]
pub async fn check_desktop_update(
    app: AppHandle,
    pending: State<'_, PendingUpdate>,
) -> Result<Option<AvailableUpdate>, String> {
    *pending.0.lock().map_err(|e| e.to_string())? = None;
    let update = app
        .updater_builder()
        .configure_client(|client| {
            client
                .https_only(true)
                .timeout(std::time::Duration::from_secs(90))
        })
        .build()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?;
    let result = update.as_ref().map(|u| AvailableUpdate {
        version: u.version.clone(),
    });
    *pending.0.lock().map_err(|e| e.to_string())? = update;
    Ok(result)
}

#[tauri::command]
pub async fn install_desktop_update(
    app: AppHandle,
    pending: State<'_, PendingUpdate>,
) -> Result<(), String> {
    // Take the checked release so duplicate clicks cannot start parallel installs.
    let update = pending
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .take()
        .ok_or("Check for updates before installing")?;
    if let Err(error) = update.download_and_install(|_, _| {}, || {}).await {
        *pending.0.lock().map_err(|e| e.to_string())? = Some(update);
        return Err(error.to_string());
    }
    app.restart();
}
