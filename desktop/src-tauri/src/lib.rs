use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{Emitter, Manager};

mod adapters;
mod builtin_skills;
pub mod cli;
mod harness;
mod library;

#[cfg(target_os = "macos")]
fn apply_transparent_dock_icon() -> Result<(), Box<dyn std::error::Error>> {
    use objc2::{AllocAnyThread, MainThreadMarker};
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::NSData;

    let main_thread = MainThreadMarker::new().ok_or("Dock icon requires the main thread")?;
    let data = NSData::with_bytes(include_bytes!("../icons/icon.png"));
    let image = NSImage::initWithData(NSImage::alloc(), &data).ok_or("Invalid Dock icon PNG")?;
    unsafe {
        NSApplication::sharedApplication(main_thread).setApplicationIconImage(Some(&image));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn hide_macos_window_controls(
    window: &tauri::WebviewWindow,
) -> Result<(), Box<dyn std::error::Error>> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSWindow, NSWindowButton};

    let _main_thread = MainThreadMarker::new().ok_or("Window controls require the main thread")?;
    let pointer = window.ns_window()?;
    if pointer.is_null() {
        return Err("Missing native window".into());
    }
    let native = unsafe { &*pointer.cast::<NSWindow>() };
    for kind in [
        NSWindowButton::CloseButton,
        NSWindowButton::MiniaturizeButton,
        NSWindowButton::ZoomButton,
    ] {
        if let Some(button) = native.standardWindowButton(kind) {
            button.setHidden(true);
        }
    }
    Ok(())
}

pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .invoke_handler(tauri::generate_handler![
            library::library_list,
            library::library_save,
            library::library_delete,
            library::library_import_image,
            library::library_skill_files,
            library::library_open_skill_folder,
            builtin_skills::creation_builtin_skills,
            harness::adapter_list,
            harness::creation_list,
            harness::creation_start,
            harness::creation_continue,
            harness::creation_revise_image,
            harness::creation_promote_showcase,
            harness::creation_cancel,
            harness::creation_open_file,
            harness::creation_directory
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            {
                apply_transparent_dock_icon()?;
                if let Some(window) = app.get_webview_window("main") {
                    hide_macos_window_controls(&window)?;
                }
            }
            app.manage(library::LibraryStore::new(
                app.path().app_data_dir()?.join("library"),
            )?);
            app.manage(harness::HarnessEngine::new(
                app.path().app_data_dir()?.join("creations"),
            )?);
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
        if let tauri::RunEvent::Exit = event {
            app.state::<harness::HarnessEngine>().shutdown();
        }
        #[cfg(target_os = "macos")]
        if let tauri::RunEvent::Reopen { .. } = event {
            if let Some(window) = app.get_webview_window("main") {
                if let Err(error) = hide_macos_window_controls(&window) {
                    eprintln!("Unable to hide window controls: {error}");
                }
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        #[cfg(not(target_os = "macos"))]
        let _ = (app, event);
    });
}
