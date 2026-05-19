// Declare modules
pub mod modules;

// Import commands from modules
use modules::commands;
use modules::process;
use modules::files;
use modules::runtime;
use modules::rpc;
use modules::journal;




#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .plugin(tauri_plugin_dialog::init())
    .plugin(tauri_plugin_shell::init())
    .setup(|app| {
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }
      Ok(())
    })
    .invoke_handler(tauri::generate_handler![
      // Commands from modules::commands
      commands::list_utxos,
      commands::broadcast_advanced_transaction,
      commands::dashboard_data,
      commands::get_receive_addresses,
      commands::new_address,
      commands::get_change_address,
      commands::get_network_mode,
      commands::send_hemp,
      commands::list_assets,
      commands::transfer_asset,
      commands::issue_asset,
      commands::ban_old_peers,
      commands::get_banned_peers,
      commands::unban_peer,
      commands::dump_priv_key,
      commands::import_priv_key,
      commands::wallet_encrypt,
      commands::wallet_unlock,
      commands::wallet_lock,
      commands::change_wallet_password,
      commands::get_net_info,
      commands::execute_ping,
      commands::check_open_port,
      commands::run_shell_command,
      commands::shell_autocomplete,
      commands::run_cli_command,
      commands::run_cli_args,
      commands::get_info,
      commands::list_address_groupings,
      commands::get_asset_data,
      commands::list_network_assets,
      commands::check_ownership_token,
      commands::reissue_asset,
      commands::lock_asset_supply,
      commands::update_asset_metadata,
      commands::issue_unique_asset,

      // Commands from modules::process
      process::start_node,
      process::stop_node,
      process::set_network_mode,
      process::restart_app,
      process::restore_wallet,
      process::create_new_wallet,

      // Commands from modules::files
      files::init_config,
      files::read_config,
      files::write_config,
      files::read_log,
      files::truncate_log,
      files::open_data_dir,
      files::backup_data_folder,
      files::backup_data_folder_to,
      files::create_default_config,
      files::get_binary_status,
      files::extract_binaries,
      files::load_address_book,
      files::save_address_book,
      files::extract_snapshot,
      files::read_text_file,
      files::write_text_file,
      files::check_config_exists,
      files::get_data_folder_info,
      files::load_app_settings,
      files::save_app_settings,
      runtime::get_runtime_status,
      runtime::probe_default_daemon,
      runtime::take_daemon_ownership,
      runtime::get_daemon_ownership,
      runtime::release_daemon_ownership,
      runtime::identify_running_daemon,
      runtime::wait_for_daemon_ready,
      rpc::rpc_get_blockchain_info,
      rpc::rpc_get_network_info,
      rpc::rpc_get_wallet_info,
      rpc::rpc_call,
      rpc::rpc_dashboard,
      journal::get_tx_journal_path,
      journal::list_tx_journal_entries,
      journal::add_tx_journal_entry,
      journal::update_tx_journal_entry,
      journal::delete_tx_journal_entry,
      journal::export_tx_journal,
      
      // Additional Commands
      commands::backup_wallet,
      commands::backup_wallet_to,
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
