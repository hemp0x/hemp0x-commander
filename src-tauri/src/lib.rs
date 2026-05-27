// Declare modules
pub mod modules;

// Import commands from modules
use modules::commands;
use modules::process;
use modules::files;
use modules::runtime;
use modules::rpc;
use modules::journal;
use modules::stratum;




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
      commands::preview_send_hemp,
      commands::preview_transfer_asset,
      commands::get_transaction_history,
      commands::list_assets,
      commands::transfer_asset,
      commands::issue_asset,
      commands::ban_old_peers,
      commands::get_banned_peers,
      commands::unban_peer,
      commands::dump_priv_key,
      commands::import_priv_key,
      commands::export_wallet_migration,
      commands::validate_wallet_migration,
      commands::restore_wallet_migration,
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
      commands::preview_issue_asset,
      commands::preview_issue_sub_asset,
      commands::preview_issue_unique_asset,
      commands::preview_reissue_asset,
      commands::preview_issue_qualifier_asset,
      commands::issue_qualifier_asset,
      commands::preview_issue_restricted_asset,
      commands::issue_restricted_asset,
      commands::preview_add_tag_to_address,
      commands::add_tag_to_address,
      commands::preview_remove_tag_from_address,
      commands::remove_tag_from_address,
      commands::check_address_tag,
      commands::list_tags_for_address,
      commands::list_addresses_for_tag,
      commands::get_verifier_string,
      commands::list_global_restrictions,
      commands::check_global_restriction,
      commands::request_snapshot,
      commands::get_snapshot_request,
      commands::list_snapshot_requests,
      commands::cancel_snapshot_request,
      commands::get_asset_snapshot,
      commands::preview_distribute_reward,
      commands::distribute_reward,
      commands::get_distribute_reward_status,

      // Raw Transaction Commands
      commands::decode_raw_transaction,
      commands::test_mempool_accept,
      commands::create_unsigned_raw_transaction,

      // Messaging Commands
      commands::get_messaging_info,
      commands::view_asset_messages,
      commands::view_message_channels,
      commands::subscribe_to_channel,
      commands::unsubscribe_from_channel,
      commands::preview_send_announcement,
      commands::send_announcement,

      // IPFS Commands
      commands::ipfs_validate,
      commands::ipfs_gateway_url,

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
      runtime::get_daemon_process_identity,
      rpc::get_rpc_auth_status,
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
      
      // Consolidation Commands
      commands::preview_wallet_consolidation,
      commands::broadcast_wallet_consolidation,
      commands::plan_wallet_consolidation,
      commands::get_policy_diagnostics,

      // Additional Commands
      commands::backup_wallet,
      commands::backup_wallet_to,

      // Stratum Commands
      stratum::start_stratum_server,
      stratum::stop_stratum_server,
      stratum::get_stratum_status,
      stratum::validate_stratum_address,
      stratum::get_stratum_bind_candidates,
      stratum::reset_stratum_stats,
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
