#![allow(dead_code)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::drop_non_drop)]
#![allow(clippy::type_complexity)]
#![allow(clippy::derivable_impls)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::manual_ok_err)]

mod auth;
mod cache;
mod commands;
mod content;
mod discord;
mod domain;
mod error;
mod hashutil;
mod java;
mod launch;
mod logs;
mod net;
mod paths;
mod protocol;
mod providers;
mod settings;
mod state;
mod window;

use tauri::Manager;

pub fn run_discord_watch(pid: u32, instance: &str) {
    discord::watch_until_exit(pid, instance);
}

pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let state = state::AppState::open().map_err(|err| err.to_string())?;
            let _ = app
                .asset_protocol_scope()
                .allow_directory(&state.paths.root, true);
            if let Some(window) = app.get_webview_window("main") {
                window::apply_native_material(&window);
                if let Ok(icon) =
                    tauri::image::Image::from_bytes(include_bytes!("../icons/icon-256.png"))
                {
                    let _ = window.set_icon(icon);
                }
            }
            crate::discord::start();
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::drag_window,
            commands::open_external,
            commands::search_catalog,
            commands::list_instances,
            commands::create_instance,
            commands::create_instance_and_install,
            commands::list_game_versions,
            commands::instance_full_path,
            commands::instance_is_running,
            commands::show_window,
            commands::is_dev,
            commands::delete_instance,
            commands::detect_java,
            commands::install_java,
            commands::install_game,
            commands::launch_game,
            commands::stop_game,
            commands::login_offline,
            commands::login_msa,
            commands::login_ely,
            commands::list_accounts,
            commands::remove_account,
            commands::logs_get_logs,
            commands::logs_get_logs_by_filename,
            commands::logs_get_output_by_filename,
            commands::logs_delete_logs_by_filename,
            commands::logs_delete_logs,
            commands::logs_get_latest_log_cursor,
            commands::should_disable_mouseover,
            commands::save_curseforge_key,
            commands::curseforge_key_set,
            commands::save_microsoft_client_id,
            commands::microsoft_client_id_set,
            commands::resolve_dependencies,
            commands::install_content,
            commands::add_project_from_version,
            commands::list_installed_content,
            commands::content_is_installed,
            commands::remove_content,
            commands::toggle_disable_content,
            commands::update_instance,
            commands::instance_options,
            commands::save_instance_options,
            commands::get_launcher_settings,
            commands::set_launcher_settings,
            commands::app_directory,
            commands::get_max_memory,
            commands::discord_set_idle,
            commands::pick_path,
            commands::save_instance_icon_png,
            commands::list_icon_gallery,
            commands::analyze_crash,
            commands::add_play_seconds,
            commands::ignore_mod_update,
            commands::get_catalog_project,
            commands::list_project_versions,
            commands::check_content_updates,
            commands::list_missing_dependencies,
            commands::install_missing_dependencies,
        ])
        .build(tauri::generate_context!());

    match app {
        Ok(app) => {
            app.run(|_handle, event| {
                if let tauri::RunEvent::ExitRequested { .. } = event {
                    crate::discord::spawn_watcher_if_playing();
                }
            });
        }
        Err(err) => {
            eprintln!("WinModing failed to start: {err}");
            std::process::exit(1);
        }
    }
}
