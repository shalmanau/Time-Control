fn main() {
    tauri_plugin::Builder::new(&["start", "poll", "wifi", "install"])
        .android_path("android")
        .build();
}
