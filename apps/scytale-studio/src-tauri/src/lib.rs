mod commands;
mod pty;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let state = pty::PtyState::spawn(app.handle())?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::create_wallet_with_pin,
            commands::get_system_status,
            commands::get_node_telemetry,
            commands::get_passbook_ledger,
            commands::get_active_account_details,
            commands::list_local_wallets,
            commands::set_active_wallet,
            commands::draft_transaction,
            commands::sign_and_broadcast_transaction,
            pty::pty_write,
            pty::pty_resize
        ])
        .run(tauri::generate_context!())
        .expect("error while running Scytale Desktop");
}
