use tauri::{
    plugin::{Builder, TauriPlugin},
    Runtime,
};
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("nearby")
        .setup(|_app, api| {
            #[cfg(target_os = "android")]
            api.register_android_plugin("app.timeledger.nearby", "NearbyPlugin")?;
            #[cfg(not(target_os = "android"))]
            let _ = api;
            Ok(())
        })
        .build()
}
