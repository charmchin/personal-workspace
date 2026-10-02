mod backup;
mod commands;
mod database;
mod error;
mod models;
mod native_lock;
mod repository;
mod restore;
mod session;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir().map_err(|error| {
                Box::<dyn std::error::Error>::from(format!("无法确定本地数据目录：{error}"))
            })?;
            let state = database::AppState::new(data_dir)
                .map_err(|error| Box::<dyn std::error::Error>::from(error.message))?;
            app.manage(state);
            native_lock::install(app.handle()).map_err(std::io::Error::other)?;
            app.manage(std::sync::Mutex::new(Some(native_lock::start_monitor(
                app.handle(),
            ))));
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
            if matches!(event, tauri::RunEvent::Exit) {
                native_lock::remove();
                let state = app.state::<database::AppState>();
                state.session.revoke("shutdown");
                state
                    .stopping
                    .store(true, std::sync::atomic::Ordering::Release);
                let monitor = app.state::<std::sync::Mutex<Option<std::thread::JoinHandle<()>>>>();
                if let Some(thread) = monitor
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner())
                    .take()
                {
                    thread.thread().unpark();
                    let _ = thread.join();
                }
            }
        });
}
