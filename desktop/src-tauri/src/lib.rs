use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{Emitter, Manager};

pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let chinese = std::env::var("LANG").unwrap_or_default().starts_with("zh");
            let check = MenuItem::with_id(
                app,
                "check-updates",
                if chinese {
                    "检查更新…"
                } else {
                    "Check for Updates…"
                },
                true,
                None::<&str>,
            )?;
            let separator = PredefinedMenuItem::separator(app)?;
            let quit = PredefinedMenuItem::quit(app, None)?;
            let app_menu =
                Submenu::with_items(app, "CreatorX", true, &[&check, &separator, &quit])?;
            let close = PredefinedMenuItem::close_window(app, None)?;
            let file = Submenu::with_items(app, "File", true, &[&close])?;
            let minimize = PredefinedMenuItem::minimize(app, None)?;
            let maximize = PredefinedMenuItem::maximize(app, None)?;
            let window = Submenu::with_items(app, "Window", true, &[&minimize, &maximize])?;
            app.set_menu(Menu::with_items(app, &[&app_menu, &file, &window])?)?;
            app.on_menu_event(|app, event| {
                if event.id().as_ref() == "check-updates" {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                        let _ = window.emit("creatorx:check-updates", ());
                    }
                }
            });
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("failed to start CreatorX");
    app.run(|app, event| {
        #[cfg(target_os = "macos")]
        if let tauri::RunEvent::Reopen { .. } = event {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        #[cfg(not(target_os = "macos"))]
        let _ = (app, event);
    });
}
