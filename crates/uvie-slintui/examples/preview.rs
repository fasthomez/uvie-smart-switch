//! Preview the Slint settings window without the tray app:
//! `cargo run -p uvie-slintui --example preview`
//! Writes to a scratch dir under %TEMP% so it doesn't touch real settings.

fn main() {
    let dir = std::env::temp_dir().join("uvie-slintui-preview");
    let tx =
        uvie_slintui::spawn_settings_thread(dir.join("settings.json"), dir.join("macros.json"));
    let _ = tx.send(());
    // The UI thread owns the event loop; just park until it exits.
    loop {
        std::thread::sleep(std::time::Duration::from_secs(60));
    }
}
