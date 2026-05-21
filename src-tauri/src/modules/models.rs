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
  pub protocolversion: u64,
  pub connections: u64,
  pub localaddresses: Vec<String>,
  pub full_ip: String,
}

#[derive(Serialize)]
pub struct DataFolderInfo {
  pub path: String,
  pub size_bytes: u64,
  pub size_display: String,
  pub config_exists: bool,
  pub wallet_exists: bool,
  pub folder_exists: bool,
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

#[derive(Serialize, serde::Deserialize)]
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
    }
  }
}

#[derive(Serialize)]
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
  pub projected_final_utxo_count: usize,
  pub max_inputs_per_round: usize,
  pub target_max_tx_bytes: u64,
  pub total_estimated_fee: String,
  pub rounds: Vec<ConsolidationRoundPlan>,
}

#[derive(Serialize)]
pub struct PolicyDiagnostics {
  pub current_safe_utxo_count: usize,
  pub max_safe_inputs_for_one_output: usize,
  pub max_safe_inputs_for_two_outputs: usize,
  pub estimated_selected_tx_bytes: u64,
  pub estimated_selected_fee: String,
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
