mod backup;
mod cancellation;
mod commands;
mod database;
mod error;
mod instance;
mod models;
mod native_lock;
mod quotes;
mod repository;
mod restore;
mod session;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .on_window_event(|window, event| {
            if window.label() == "main"
                && let tauri::WindowEvent::CloseRequested { api, .. } = event
            {
                // A single-window workbench has no tray/background workflow.
                // Exit through RunEvent::Exit so workers, keys and final snapshots close safely.
                api.prevent_close();
                window.app_handle().exit(0);
            }
        })
        .setup(|app| {
            let data_dir = app.path().app_data_dir().map_err(|error| {
                Box::<dyn std::error::Error>::from(format!("无法确定本地数据目录：{error}"))
            })?;
            let state = match database::AppState::new(data_dir) {
                Ok(state) => state,
                Err(error) if error.code == "APP_ALREADY_RUNNING" => {
                    native_lock::request_focus();
                    app.handle().exit(0);
                    return Ok(());
                }
                Err(error) => return Err(Box::<dyn std::error::Error>::from(error.message)),
            };
            app.manage(state);
            native_lock::install(app.handle()).map_err(std::io::Error::other)?;
            app.manage(std::sync::Mutex::new(vec![
                native_lock::start_monitor(app.handle()),
                native_lock::start_snapshot_monitor(app.handle()),
            ]));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::security_status,
            commands::security_unlock,
            commands::security_initialize_with_password,
            commands::security_unlock_with_password,
            commands::security_change_password,
            commands::security_lock,
            commands::security_activity,
            commands::get_dashboard,
            commands::list_tasks,
            commands::upsert_task,
            commands::toggle_task,
            commands::list_calendar_items,
            commands::upsert_calendar_item,
            commands::list_projects,
            commands::upsert_project,
            commands::list_work_logs,
            commands::upsert_work_log,
            commands::create_task_from_work_log,
            commands::list_goals,
            commands::upsert_goal,
            commands::list_habits,
            commands::upsert_habit,
            commands::check_habit,
            commands::list_learning_items,
            commands::upsert_learning_item,
            commands::list_content_items,
            commands::upsert_content_item,
            commands::add_content_metric,
            commands::list_investment_accounts,
            commands::upsert_investment_account,
            commands::list_instruments,
            commands::upsert_instrument,
            commands::list_portfolio_transactions,
            commands::upsert_portfolio_transaction,
            commands::upsert_price_point,
            commands::get_portfolio_snapshot,
            commands::list_reviews,
            commands::upsert_review,
            commands::generate_weekly_summary,
            commands::delete_record,
            commands::open_saved_file,
            commands::open_license_notices,
            commands::search_records,
            commands::get_settings,
            commands::update_settings,
            commands::configure_tushare_token,
            commands::has_tushare_token,
            commands::refresh_tushare_quotes,
            commands::import_portfolio_csv,
            commands::export_portfolio_csv_template,
            commands::import_ics,
            commands::export_ics,
            commands::export_backup,
            commands::restore_backup,
            commands::list_backups,
            commands::delete_backup,
            commands::restore_snapshot,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            #[cfg(target_os = "macos")]
            if matches!(event, tauri::RunEvent::Reopen { .. })
                && let Some(window) = app.get_webview_window("main")
            {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
            if matches!(event, tauri::RunEvent::Exit) {
                native_lock::remove();
                let Some(state) = app.try_state::<database::AppState>() else {
                    return;
                };
                state
                    .stopping
                    .store(true, std::sync::atomic::Ordering::Release);
                let monitor = app.state::<std::sync::Mutex<Vec<std::thread::JoinHandle<()>>>>();
                for thread in monitor
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner())
                    .drain(..)
                {
                    thread.thread().unpark();
                    let _ = thread.join();
                }
                let _ = state.refresh_snapshot_if_due(true);
                state.session.revoke("shutdown");
            }
        });
}
