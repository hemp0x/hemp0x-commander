use serde::Serialize;
// use std::collections::HashMap; // Unused


#[derive(Serialize)]
pub struct NodeInfo {
  pub state: String,
  pub blocks: u64,
  pub headers: u64,
  pub peers: u64,
  pub diff: String,
  pub synced: bool,
}

#[derive(Serialize)]
pub struct WalletInfo {
  pub balance: String,
  pub pending: String,
  pub staked: String,
  pub status: String,
}

#[derive(Serialize)]
pub struct TxItem {
  pub date: String,
  #[serde(rename = "type")]
  pub tx_type: String,
  pub amount: String,
  pub conf: u64,
  pub txid: String,
}

#[derive(Serialize)]
pub struct DashboardData {
  pub node: NodeInfo,
  pub wallet: WalletInfo,
  pub tx: Vec<TxItem>,
}

#[derive(Serialize)]
pub struct ConfigPaths {
  pub data_dir: String,
  pub config_path: String,
  pub daemon_path: String,
  pub cli_path: String,
}

#[derive(Serialize)]
pub struct BinaryStatus {
  pub daemon_exists: bool,
  pub cli_exists: bool,
  pub tx_exists: bool,
  pub daemon_path: String,
  pub cli_path: String,
  pub tx_path: String,
}

#[derive(Serialize)]
pub struct AddressItem {
  pub label: String,
  pub address: String,
  pub balance: String,
}

#[derive(Serialize)]
pub struct AssetItem {
  pub name: String,
  pub balance: String,
  #[serde(rename = "type")]
  pub asset_type: String,
  pub asset_type_label: Option<String>,
}

#[derive(Serialize, serde::Deserialize)]
pub struct UtxoItem {
  pub txid: String,
  pub vout: u64,
  pub address: Option<String>,
  pub amount: f64,
  pub confirmations: u64,
  pub spendable: Option<bool>,
  pub solvable: Option<bool>,
  pub desc: Option<String>,
  pub safe: Option<bool>,
  #[serde(default)]
  pub label: Option<String>,
  #[serde(default)]
  #[serde(rename = "scriptPubKey")]
  pub script_pub_key: Option<String>,
  #[serde(default)]
  pub asset: Option<String>,
  #[serde(default)]
  pub asset_amount: Option<f64>,
}

#[derive(Serialize, serde::Deserialize, Clone)]
pub struct AddressBookEntry {
  pub label: String,
  pub address: String,
  pub locked: bool,
  pub date: u64,
}

#[derive(Serialize)]
pub struct AssetData {
  pub name: String,
  pub amount: f64,
  pub units: u8,
  pub reissuable: bool,
  pub has_ipfs: bool,
  pub ipfs_hash: String,
  pub block_height: u64,
}

#[derive(Serialize)]
pub struct BanResult {
  pub banned_count: u32,
  pub banned_peers: Vec<String>,
}

#[derive(Serialize)]
pub struct BanEntry {
  pub address: String,
  pub banned_until: String,
  pub ban_reason: String,
}

#[derive(Serialize)]
pub struct NetworkInfo {
  pub version: u64,
  pub subversion: String,
  pub build: String,
  pub build_commit: String,
  pub protocolversion: u64,
  pub connections: u64,
  pub localaddresses: Vec<String>,
  pub full_ip: String,
}

#[derive(Serialize)]
pub struct DataFolderInfo {
  pub path: String,
  pub default_path: String,
  pub using_custom_path: bool,
  pub commander_settings_path: String,
  pub bootstrap_path: String,
  pub size_bytes: u64,
  pub size_display: String,
  pub config_exists: bool,
  pub wallet_exists: bool,
  pub folder_exists: bool,
  pub blocks_exists: bool,
  pub chainstate_exists: bool,
  pub debug_log_exists: bool,
  pub lock_exists: bool,
  pub bootstrap_error: Option<String>,
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub struct RawTxInput {
  pub txid: String,
  pub vout: u64,
}

#[derive(Serialize, Debug)]
pub struct SendPreview {
  pub destination: String,
  pub amount: String,
  pub asset: String,
  pub available_balance: String,
  pub fee_estimate: Option<String>,
  pub fee_warning: Option<String>,
  pub warnings: Vec<String>,
  pub summary: String,
  pub validated: bool,
}

#[derive(Serialize)]
pub struct DataMovePreview {
  pub source_path: String,
  pub target_path: String,
  pub source_size_bytes: u64,
  pub source_size_display: String,
  pub target_exists: bool,
  pub target_is_empty: bool,
  pub target_has_files: bool,
  pub wallet_present: bool,
  pub config_present: bool,
  pub blocks_present: bool,
  pub chainstate_present: bool,
  pub warnings: Vec<String>,
}

#[derive(Serialize)]
pub struct DataMoveResult {
  pub success: bool,
  pub message: String,
  pub files_copied: u64,
  pub bytes_copied: u64,
}

#[derive(Serialize)]
pub struct RepairModeInfo {
  pub active_mode: String,
  pub description: String,
  pub requires_restart: bool,
}

#[derive(Serialize, Clone)]
pub struct RepairStatus {
  pub active: bool,
  pub mode: Option<String>,
  pub phase: String,
  pub rpc_online: bool,
  pub lock_exists: bool,
  pub blocks: Option<u64>,
  pub headers: Option<u64>,
  pub verification_progress: Option<f64>,
  pub latest_log_line: Option<String>,
  pub log_hint: Option<String>,
}

#[derive(Serialize, serde::Deserialize, Clone)]
#[serde(default)]
pub struct AppSettings {
  pub hide_balance: bool,
  pub hide_activity: bool,
  pub show_welcome: bool,
  pub hidden_assets: Vec<String>,
  pub asset_order: Vec<String>,
  pub auto_start_daemon_on_launch: bool,
  pub keep_daemon_running_on_close: bool,
  pub allow_non_bundled_core_next: bool,
  pub auto_peer_protection_enabled: bool,
  pub custom_data_dir: Option<String>,
  pub custom_core_binary_dir: Option<String>,
  pub pending_repair_mode: Option<String>,
  pub active_repair_mode: Option<String>,
  pub active_repair_started_at: Option<u64>,
}

#[derive(Serialize)]
pub struct IpfsReferenceInfo {
  pub normalized: String,
  pub kind: String,
  pub warnings: Vec<String>,
}

impl Default for AppSettings {
  fn default() -> Self {
    Self {
      hide_balance: false,
      hide_activity: false,
      show_welcome: true,
      hidden_assets: Vec::new(),
      asset_order: Vec::new(),
      auto_start_daemon_on_launch: false,
      keep_daemon_running_on_close: false,
      allow_non_bundled_core_next: false,
      auto_peer_protection_enabled: true,
      custom_data_dir: None,
      custom_core_binary_dir: None,
      pending_repair_mode: None,
      active_repair_mode: None,
      active_repair_started_at: None,
    }
  }
}

#[derive(Serialize, Debug)]
pub struct IssuePreview {
  pub operation_type: String,
  pub asset_name: String,
  pub qty: Option<String>,
  pub units: Option<u8>,
  pub reissuable: Option<bool>,
  pub ipfs_hash: Option<String>,
  pub parent_asset: Option<String>,
  pub tags: Option<Vec<String>>,
  pub is_irreversible: bool,
  pub warnings: Vec<String>,
  pub summary: String,
  pub validated: bool,
}

#[derive(Serialize, Debug)]
pub struct ConsolidationPreview {
  pub utxo_count: usize,
  pub input_total: String,
  pub estimated_bytes: u64,
  pub fee_rate_sat_per_byte: u64,
  pub fee_estimate: String,
  pub output_amount: String,
  pub destination: String,
  pub warnings: Vec<String>,
  pub summary: String,
  pub utxos: Vec<ConsolidationUtxoEntry>,
}

#[derive(Serialize, Clone, Debug)]
pub struct ConsolidationUtxoEntry {
  pub txid: String,
  pub vout: u64,
  pub amount: String,
  pub address: Option<String>,
  pub confirmations: u64,
  pub spendable: bool,
  pub safe: bool,
  pub asset: Option<String>,
  pub asset_amount: Option<f64>,
}

#[derive(Serialize)]
pub struct TransactionHistoryItem {
  pub txid: String,
  pub date: String,
  #[serde(rename = "type")]
  pub tx_type: String,
  pub amount: String,
  pub confirmations: u64,
  pub address: Option<String>,
  pub asset: Option<String>,
  pub fee: Option<String>,
  pub raw: Option<serde_json::Value>,
}

#[derive(Serialize)]
pub struct TransactionHistoryResult {
  pub items: Vec<TransactionHistoryItem>,
  pub total: usize,
  pub has_more: bool,
}

#[derive(Serialize)]
pub struct QualifierIssuePreview {
  pub operation_type: String,
  pub asset_name: String,
  pub qualifier_name: String,
  pub qty: String,
  pub destination: Option<String>,
  pub ipfs_hash: Option<String>,
  pub warnings: Vec<String>,
  pub summary: String,
  pub is_irreversible: bool,
  pub validated: bool,
}

#[derive(Serialize)]
pub struct RestrictedIssuePreview {
  pub operation_type: String,
  pub asset_name: String,
  pub qty: String,
  pub verifier: String,
  pub destination: String,
  pub units: u8,
  pub reissuable: bool,
  pub ipfs_hash: Option<String>,
  pub warnings: Vec<String>,
  pub summary: String,
  pub is_irreversible: bool,
  pub validated: bool,
}

#[derive(Serialize)]
pub struct TagOperationPreview {
  pub operation_type: String,
  pub asset_name: String,
  pub tag_name: String,
  pub address: String,
  pub is_adding: bool,
  pub warnings: Vec<String>,
  pub summary: String,
  pub is_irreversible: bool,
  pub validated: bool,
}

#[derive(Serialize)]
pub struct SnapshotRequestPreview {
  pub asset_name: String,
  pub block_height: i64,
  pub warnings: Vec<String>,
  pub summary: String,
}

#[derive(Serialize)]
pub struct SnapshotRequestEntry {
  pub asset_name: String,
  pub block_height: i64,
}

#[derive(Serialize)]
pub struct SnapshotOwnerEntry {
  pub address: String,
  pub amount_owned: serde_json::Value,
}

#[derive(Serialize)]
pub struct SnapshotData {
  pub name: String,
  pub height: i64,
  pub owners: Vec<SnapshotOwnerEntry>,
}

#[derive(Serialize)]
pub struct ConsolidationRoundPlan {
  pub round_number: u32,
  pub input_count: usize,
  pub input_total: String,
  pub estimated_bytes: u64,
  pub fee_estimate: String,
  pub projected_output: String,
  pub selected_outpoints: Vec<String>,
}

#[derive(Serialize)]
pub struct ConsolidationPlan {
  pub initial_utxo_count: usize,
  pub selected_safe_utxo_count: usize,
  pub target_final_utxo_count: usize,
  pub projected_final_utxo_count: usize,
  pub estimated_round_count: usize,
  pub planned_round_count: usize,
  pub max_inputs_per_round: usize,
  pub target_max_tx_bytes: u64,
  pub total_estimated_fee: String,
  pub total_estimated_bytes: u64,
  pub rounds: Vec<ConsolidationRoundPlan>,
}

#[derive(Serialize)]
pub struct PolicyDiagnostics {
  pub current_safe_utxo_count: usize,
  pub max_safe_inputs_for_one_output: usize,
  pub max_safe_inputs_for_two_outputs: usize,
  pub estimated_selected_tx_bytes: u64,
  pub estimated_selected_fee: String,
  pub fee_rate_sat_per_byte: u64,
}

#[derive(Serialize)]
pub struct RawTxBuildResult {
  pub raw_hex: String,
  pub decoded: serde_json::Value,
  pub input_count: usize,
  pub output_count: usize,
  pub fee_warning: String,
}

#[derive(Serialize)]
pub struct RewardDistributionPreview {
  pub operation_type: String,
  pub asset_name: String,
  pub ownership_asset: String,
  pub snapshot_height: i64,
  pub distribution_asset: String,
  pub gross_amount: String,
  pub exception_addresses: Option<String>,
  pub estimated_recipient_count: Option<usize>,
  pub warnings: Vec<String>,
  pub summary: String,
  pub is_irreversible: bool,
  pub validated: bool,
}

#[derive(Serialize)]
pub struct AssetMessageEntry {
  pub asset_name: String,
  pub message: String,
  pub time: String,
  pub block_height: i64,
  pub status: String,
  pub expire_time: Option<String>,
  pub expire_utc_time: Option<i64>,
  pub txid: Option<String>,
  pub channel: Option<String>,
  pub authority_asset: Option<String>,
  pub authority_address: Option<String>,
  pub block_hash: Option<String>,
  pub sender_address: Option<String>,
}

#[derive(Serialize)]
pub struct MessagingInfo {
  pub enabled: bool,
  pub messaging_active: bool,
  pub restricted_active: bool,
  pub activation_block: i64,
  pub databases_available: bool,
  pub caches_available: bool,
  pub message_count: i64,
  pub channel_count: i64,
  pub dirty_cache_size_bytes: i64,
  pub wallet_available: bool,
  pub warnings: Vec<String>,
}

#[derive(Serialize)]
pub struct AssetAnnouncementPreview {
  pub channel_name: String,
  pub ipfs_hash: String,
  pub expire_time: Option<i64>,
  pub has_ownership: bool,
  pub is_irreversible: bool,
  pub warnings: Vec<String>,
  pub summary: String,
  pub validated: bool,
}
