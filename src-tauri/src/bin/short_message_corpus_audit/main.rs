// short_message_corpus_audit
//
// Dev-only binary that drives `app_lib::modules::short_message::encode` over
// a large corpus (built-in plus one or more on-disk corpus directories) and
// emits a coverage/audit report.
//
// Reports are written to:
//   untracked/commander-v1.4/short-message-corpus-audit/
//
// If --run-label LABEL is given, reports go under:
//   <out-dir>/runs/<LABEL>/
//
// CLI:
//   --corpus-dir PATH    Load every `*.txt` file in PATH. May be repeated.
//   --no-built-in        Skip the in-code built-in corpus.
//   --out-dir PATH       Override the report output directory.
//   --run-label LABEL    Write reports under <out-dir>/runs/<LABEL>/.
//   --clean-out-dir      Remove the run output directory before writing.
//                        Only the run directory is touched.
//   <file>...            One or more individual sample files.
//
// Reports are deterministic given the same input corpus and the same
// compiled `HOXSHT` table pack. The `Table Identity` section in the report
// includes a SHA-256 fingerprint of the current dictionary/suffix/alphabet
// data so future runs can be compared exactly.

use app_lib::modules::short_message::{decode, encode};
use app_lib::modules::short_message_tables::{
    table_identity, DICTIONARIES, HOXSHT_VERSION_MARKER, SUFFIXES,
};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

mod corpus;

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
enum Category {
    CasualChat,
    BusinessPayment,
    OperationsSupport,
    AssetHolder,
    LogisticsTrace,
    CryptoHemp0x,
    EdgeCases,
}

impl Category {
    fn label(self) -> &'static str {
        match self {
            Category::CasualChat => "casual_chat",
            Category::BusinessPayment => "business_payment",
            Category::OperationsSupport => "operations_support",
            Category::AssetHolder => "asset_holder",
            Category::LogisticsTrace => "logistics_trace",
            Category::CryptoHemp0x => "crypto_hemp0x",
            Category::EdgeCases => "edge_cases",
        }
    }
}

#[derive(Clone, Debug)]
struct Sample {
    text: String,
    category: Category,
    source: String,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
enum Outcome {
    Fit {
        mode: String,
        dict_name: Option<String>,
        dict_idx: Option<u8>,
        payload_len: usize,
        hex: String,
        warnings: Vec<String>,
        roundtrip_text: String,
        roundtrip_ok: bool,
    },
    Fail {
        reason: String,
    },
}

#[derive(Debug, Serialize, Clone)]
struct SampleReport {
    text: String,
    category: String,
    source: String,
    outcome: String,
    mode: Option<String>,
    dict_name: Option<String>,
    payload_len: Option<usize>,
    roundtrip_text: Option<String>,
    roundtrip_ok: Option<bool>,
    warnings: Option<Vec<String>>,
    failure_reason: Option<String>,
}

#[derive(Debug, Serialize)]
struct Summary {
    total_samples: usize,
    fit_count: usize,
    fail_count: usize,
    fit_pct: f64,
    fail_pct: f64,
    dict_mode_count: usize,
    dict_mode_pct: f64,
    fallback_by_mode: BTreeMap<String, usize>,
    avg_payload_len: f64,
    p50_payload_len: usize,
    p90_payload_len: usize,
    p95_payload_len: usize,
    min_payload_len: usize,
    max_payload_len: usize,
    near_limit_count: usize,
    near_limit_pct: f64,
}

#[derive(Debug, Serialize)]
struct DictStats {
    name: String,
    count: usize,
    pct_of_dict: f64,
    avg_payload_len: f64,
    sample_categories: BTreeMap<String, usize>,
}

#[derive(Debug, Serialize)]
struct TableIdentityReport {
    version_marker: String,
    dictionary_count: usize,
    entry_counts: Vec<(String, usize)>,
    suffix_count: usize,
    fingerprint_sha256: String,
}

#[derive(Debug, Serialize)]
struct FailureEntry {
    text: String,
    category: String,
    source: String,
    raw_len: usize,
    reason: String,
}

#[derive(Debug, Serialize)]
struct NonDictEntry {
    text: String,
    category: String,
    source: String,
    mode: String,
    payload_len: usize,
    warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
struct WorstFitEntry {
    text: String,
    category: String,
    source: String,
    mode: String,
    dict_name: Option<String>,
    payload_len: usize,
}

#[derive(Debug, Serialize)]
struct MissByCategory {
    category: String,
    count: usize,
    fallback_modes: BTreeMap<String, usize>,
    top_missing_words: Vec<(String, usize)>,
}

#[derive(Debug, Serialize)]
struct MissBySource {
    source: String,
    count: usize,
    fallback_modes: BTreeMap<String, usize>,
    top_missing_words: Vec<(String, usize)>,
}

#[derive(Debug, Serialize)]
struct SourceSummary {
    source: String,
    category: String,
    total: usize,
    fit: usize,
    fail: usize,
    fit_pct: f64,
    dict: usize,
    dict_pct: f64,
    fallback_by_mode: BTreeMap<String, usize>,
    avg_payload_len: f64,
}

#[derive(Debug, Serialize)]
struct CategorySummary {
    category: String,
    total: usize,
    fit: usize,
    fail: usize,
    fit_pct: f64,
    fail_pct: f64,
    dict: usize,
    dict_pct: f64,
    fallback_by_mode: BTreeMap<String, usize>,
    avg_payload_len: f64,
}

#[derive(Debug, Serialize, Clone)]
struct NgramCandidate {
    surface: String,
    count: usize,
    kind: String,
    suggested_dict: String,
    source_count: usize,
    category_count: usize,
    only_synthetic: bool,
    likely_id_or_noise: bool,
    likely_too_long: bool,
}

#[derive(Debug, Serialize)]
struct CrossLeak {
    surface: String,
    count: usize,
    current_dict: String,
}

#[derive(Debug, Serialize)]
struct LowValueToken {
    surface: String,
    dict: String,
    corpus_occurrences: usize,
}

#[derive(Debug, Serialize)]
struct WatchlistReport {
    should_stay_in_dict_mode: Vec<String>,
    should_fit_even_if_not_dict: Vec<String>,
}

#[derive(Debug, Serialize)]
struct CorpusSources {
    built_in_samples: usize,
    per_file: BTreeMap<String, usize>,
    duplicates_removed: usize,
    total_unique: usize,
    corpus_dirs: Vec<String>,
    input_files: Vec<String>,
    no_built_in: bool,
}

#[derive(Debug, Serialize)]
struct CoverageScorecard {
    fit_pct: f64,
    dict_pct: f64,
    fallback_pct: BTreeMap<String, f64>,
    failure_pct: f64,
    avg_payload_len: f64,
    p50_payload_len: usize,
    p90_payload_len: usize,
    p95_payload_len: usize,
    near_limit_count: usize,
    near_limit_pct: f64,
}

#[derive(Debug, Serialize)]
struct AuditReport {
    generated_at_unix: u64,
    generated_at_iso: String,
    run_label: Option<String>,
    corpus_sources: CorpusSources,
    table_identity: TableIdentityReport,
    summary: Summary,
    coverage_scorecard: CoverageScorecard,
    dictionary_distribution: Vec<DictStats>,
    source_summaries: Vec<SourceSummary>,
    category_summaries: Vec<CategorySummary>,
    worst_fitting: Vec<WorstFitEntry>,
    failures: Vec<FailureEntry>,
    non_dictionary: Vec<NonDictEntry>,
    miss_by_category: Vec<MissByCategory>,
    miss_by_source: Vec<MissBySource>,
    ngram_candidates: Vec<NgramCandidate>,
    do_not_tune_yet: Vec<NgramCandidate>,
    cross_dictionary_leakage: Vec<CrossLeak>,
    low_value_tokens: Vec<LowValueToken>,
    regression_watchlist: WatchlistReport,
    next_data_suggestions: Vec<String>,
    samples: Vec<SampleReport>,
}

fn run_sample(sample: &Sample) -> Outcome {
    let enc = match encode(&sample.text) {
        Ok(enc) => enc,
        Err(err) => {
            return Outcome::Fail {
                reason: err.to_string(),
            };
        }
    };

    if !enc.fits {
        return Outcome::Fail {
            reason: format!(
                "encoder returned fits=false, payload_len={}",
                enc.encoded_payload_len
            ),
        };
    }

    let dec = decode(&enc.hex);
    let (roundtrip_text, roundtrip_ok) = match dec.text.as_deref() {
        Some(text) if !text.is_empty() => (text.to_string(), true),
        Some(_) => (String::new(), false),
        None => (String::new(), false),
    };

    Outcome::Fit {
        mode: enc.encoding_mode.clone(),
        dict_name: enc.dictionary_name.clone(),
        dict_idx: enc.dictionary_index,
        payload_len: enc.encoded_payload_len,
        hex: enc.hex.clone(),
        warnings: enc.warnings.clone(),
        roundtrip_text,
        roundtrip_ok,
    }
}

fn collect_built_in() -> Vec<Sample> {
    corpus::built_in_corpus()
}

fn category_from_source(source: &str) -> Category {
    let s = source.to_lowercase();
    if s.contains("casual") || s.contains("chat") || s.contains("everyday") {
        Category::CasualChat
    } else if s.contains("business") || s.contains("commerce") || s.contains("sales") {
        Category::BusinessPayment
    } else if s.contains("operations") || s.contains("ops") || s.contains("support") {
        Category::OperationsSupport
    } else if s.contains("asset") || s.contains("holder") || s.contains("ipfs") || s.contains("nft")
    {
        Category::AssetHolder
    } else if s.contains("logistics")
        || s.contains("trace")
        || s.contains("provenance")
        || s.contains("harvest")
    {
        Category::LogisticsTrace
    } else if s.contains("crypto")
        || s.contains("hemp0x")
        || s.contains("wallet")
        || s.contains("forum")
    {
        Category::CryptoHemp0x
    } else if s.contains("edge") {
        Category::EdgeCases
    } else {
        Category::EdgeCases
    }
}

fn load_extra_file(path: &Path, default_source_label: &str) -> Result<Vec<Sample>, String> {
    let text = fs::read_to_string(path)
        .map_err(|e| format!("failed to read {}: {}", path.display(), e))?;
    let source_label = default_source_label.to_string();
    let category = category_from_source(default_source_label);
    let mut out = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        out.push(Sample {
            text: line.to_string(),
            category,
            source: source_label.clone(),
        });
    }
    Ok(out)
}

fn load_corpus_dir(dir: &Path) -> Result<Vec<Sample>, String> {
    let mut out = Vec::new();
    let entries = fs::read_dir(dir).map_err(|e| format!("read_dir {}: {}", dir.display(), e))?;
    let mut files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|d| d.path()))
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .and_then(|s| s.to_str())
                    .map(|s| s.eq_ignore_ascii_case("txt"))
                    .unwrap_or(false)
        })
        .collect();
    files.sort();
    for path in files {
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.eq_ignore_ascii_case("all.txt"))
            .unwrap_or(false)
        {
            continue;
        }
        let label = path
            .strip_prefix("untracked/commander-v1.4")
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let samples = load_extra_file(&path, &label)?;
        out.extend(samples);
    }
    Ok(out)
}

fn dedup(samples: Vec<Sample>) -> (Vec<Sample>, usize) {
    let mut seen: HashSet<String> = HashSet::new();
    let mut out = Vec::with_capacity(samples.len());
    let mut removed = 0usize;
    for s in samples {
        if seen.insert(s.text.clone()) {
            out.push(s);
        } else {
            removed += 1;
        }
    }
    (out, removed)
}

fn percentile(sorted: &[usize], p: f64) -> usize {
    if sorted.is_empty() {
        return 0;
    }
    let rank = (p * (sorted.len() as f64 - 1.0)).round() as usize;
    sorted[rank.min(sorted.len() - 1)]
}

fn pct(n: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        (n as f64) * 100.0 / (total as f64)
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn now_iso8601() -> String {
    use chrono::Local;
    let dt = Local::now();
    dt.format("%Y-%m-%dT%H:%M:%S%z").to_string()
}

fn timestamp_stamp() -> String {
    use chrono::Local;
    Local::now().format("%Y-%m-%d-%H%M%S").to_string()
}

fn normalize_text(s: &str) -> String {
    s.to_lowercase()
}

fn tokenize_lower(s: &str) -> Vec<String> {
    normalize_text(s)
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| t.to_string())
        .collect()
}

fn is_synthetic_source(source: &str) -> bool {
    let s = source.to_lowercase();
    if s.contains("short-message-corpus-sources/normalized")
        || s.contains("short-message-corpus-sources\\normalized")
    {
        return false;
    }
    if s.contains("short-message-corpus-sources/user-import")
        || s.contains("short-message-corpus-sources\\user-import")
    {
        return false;
    }
    s.contains("built-in")
        || s.contains("synthetic")
        || s.contains("short-message-corpus/")
        || s.contains("short-message-corpus\\")
        || s.contains("public-style/")
        || s.contains("public-style\\")
        || s.contains("normalized/")
        || s.contains("normalized\\")
        || s.contains("gen_short_message")
        || s.contains("gen_public_style")
        || s.starts_with("(file:")
}

fn is_likely_id_or_noise(s: &str) -> bool {
    if s.is_empty() {
        return true;
    }
    if s.len() > 32 {
        return true;
    }
    let alnum = s.chars().filter(|c| c.is_alphanumeric()).count();
    if alnum == 0 {
        return true;
    }
    // All-digit or hex-looking strings: high likelihood of being an ID, a
    // date, a number, or a token-like fragment.
    let all_digit_or_hex = s.chars().all(|c| c.is_ascii_hexdigit());
    if all_digit_or_hex && s.len() >= 4 {
        return true;
    }
    if s.chars().all(|c| c.is_ascii_digit()) && s.len() >= 4 {
        return true;
    }
    // Pure punctuation or single-char entries.
    if s.chars().all(|c| !c.is_alphanumeric()) {
        return true;
    }
    false
}

fn classify_word(word: &str) -> String {
    let w = word.to_lowercase();
    let crypto = [
        "btc", "eth", "usdt", "hemp", "hemp0x", "wallet", "tx", "txid", "hash", "nonce", "block",
        "mempool", "pool", "miner", "stake", "swap", "bridge", "airdrop", "node", "rpc", "pow",
        "pos", "defi", "dex", "cex", "utxo", "fee", "kyc", "aml", "api", "cli", "seed", "ledger",
        "mainnet", "testnet", "explorer", "endpoint",
    ];
    let asset = [
        "nft",
        "mint",
        "minted",
        "holder",
        "holders",
        "ipfs",
        "cid",
        "collection",
        "metadata",
        "artwork",
        "edition",
        "announcement",
        "owner",
        "claim",
        "reveal",
        "drop",
        "file",
        "document",
    ];
    let logi = [
        "shipment",
        "ship",
        "shipping",
        "tracking",
        "track",
        "warehouse",
        "carrier",
        "courier",
        "freight",
        "container",
        "pallet",
        "parcel",
        "manifest",
        "consignee",
        "consignor",
        "deliver",
        "delivery",
        "dispatch",
        "pickup",
        "transit",
        "package",
        "receive",
        "received",
        "origin",
        "destination",
        "harvest",
        "lab",
        "coa",
        "supplier",
    ];
    let biz = [
        "invoice",
        "receipt",
        "refund",
        "payment",
        "paid",
        "due",
        "balance",
        "deposit",
        "tax",
        "discount",
        "quote",
        "estimate",
        "order",
        "purchase",
        "sale",
        "vendor",
        "customer",
        "client",
        "wholesale",
        "retail",
        "billing",
        "po",
        "sku",
        "warranty",
    ];
    let ops = [
        "ticket",
        "task",
        "project",
        "status",
        "deadline",
        "milestone",
        "approval",
        "review",
        "assign",
        "report",
        "kpi",
        "sla",
        "sprint",
        "standup",
        "deploy",
        "build",
        "fix",
        "bug",
        "incident",
        "retro",
        "rollback",
        "hotfix",
        "patch",
    ];
    if crypto.iter().any(|k| w == *k || w.starts_with(k)) {
        return "H".to_string();
    }
    if asset.iter().any(|k| w == *k || w.starts_with(k)) {
        return "F".to_string();
    }
    if logi.iter().any(|k| w == *k || w.starts_with(k)) {
        return "G".to_string();
    }
    if biz.iter().any(|k| w == *k || w.starts_with(k)) {
        return "D".to_string();
    }
    if ops.iter().any(|k| w == *k || w.starts_with(k)) {
        return "C".to_string();
    }
    "A/B".to_string()
}

fn classify_phrase(phrase: &str) -> String {
    let tokens: Vec<&str> = phrase.split_whitespace().collect();
    if tokens.is_empty() {
        return "A/B".to_string();
    }
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for t in tokens {
        *counts.entry(classify_word(t)).or_insert(0) += 1;
    }
    counts
        .into_iter()
        .max_by_key(|(_, v)| *v)
        .map(|(k, _)| k)
        .unwrap_or_else(|| "A/B".to_string())
}

fn is_synthetic_sample(sample: &Sample) -> bool {
    is_synthetic_source(&sample.source)
}

#[derive(Default, Debug)]
struct MissAggregates {
    by_source_word: HashMap<String, HashMap<String, usize>>,
    by_category_word: HashMap<String, HashMap<String, usize>>,
    by_source_mode: HashMap<String, BTreeMap<String, usize>>,
    by_category_mode: HashMap<String, BTreeMap<String, usize>>,
}

fn build_report(
    samples: &[Sample],
    outcomes: &[Outcome],
    sources: &CorpusSources,
    run_label: &Option<String>,
) -> AuditReport {
    let total = samples.len();

    let mut fit_count = 0usize;
    let mut fail_count = 0usize;
    let mut dict_count = 0usize;
    let mut fallback_by_mode: BTreeMap<String, usize> = BTreeMap::new();
    let mut payload_lengths: Vec<usize> = Vec::new();
    let mut near_limit_count = 0usize;
    let mut dict_payload_sums: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut category_by_dict_total: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();

    let mut failures: Vec<FailureEntry> = Vec::new();
    let mut non_dict: Vec<NonDictEntry> = Vec::new();
    let mut worst_fitting: Vec<WorstFitEntry> = Vec::new();
    let mut sample_reports: Vec<SampleReport> = Vec::with_capacity(total);

    let mut missing_word_counts: HashMap<String, usize> = HashMap::new();
    let mut missing_bigram_counts: HashMap<String, usize> = HashMap::new();
    let mut missing_trigram_counts: HashMap<String, usize> = HashMap::new();
    let mut missing_fourgram_counts: HashMap<String, usize> = HashMap::new();
    let mut word_to_sources: HashMap<String, HashSet<String>> = HashMap::new();
    let mut word_to_categories: HashMap<String, HashSet<String>> = HashMap::new();
    let mut word_to_synthetic_count: HashMap<String, usize> = HashMap::new();
    let mut word_to_nonsynth_count: HashMap<String, usize> = HashMap::new();

    let mut aggr = MissAggregates::default();

    // Per-source and per-category accumulators for summaries.
    let mut source_total: HashMap<String, (usize, Category)> = HashMap::new();
    let mut source_fit: HashMap<String, usize> = HashMap::new();
    let mut source_fail: HashMap<String, usize> = HashMap::new();
    let mut source_dict: HashMap<String, usize> = HashMap::new();
    let mut source_payload_sum: HashMap<String, (usize, usize)> = HashMap::new();
    let mut source_modes: HashMap<String, BTreeMap<String, usize>> = HashMap::new();

    let mut cat_total: HashMap<String, usize> = HashMap::new();
    let mut cat_fit: HashMap<String, usize> = HashMap::new();
    let mut cat_fail: HashMap<String, usize> = HashMap::new();
    let mut cat_dict: HashMap<String, usize> = HashMap::new();
    let mut cat_payload_sum: HashMap<String, (usize, usize)> = HashMap::new();
    let mut cat_modes: HashMap<String, BTreeMap<String, usize>> = HashMap::new();

    for (sample, outcome) in samples.iter().zip(outcomes.iter()) {
        let st_entry = source_total
            .entry(sample.source.clone())
            .or_insert((0, sample.category));
        st_entry.0 += 1;
        *cat_total
            .entry(sample.category.label().to_string())
            .or_insert(0) += 1;

        match outcome {
            Outcome::Fit {
                mode,
                dict_name,
                dict_idx: _,
                payload_len,
                hex: _,
                warnings,
                roundtrip_text,
                roundtrip_ok,
            } => {
                fit_count += 1;
                payload_lengths.push(*payload_len);
                if *payload_len >= 24 {
                    near_limit_count += 1;
                }
                *source_fit.entry(sample.source.clone()).or_insert(0) += 1;
                *cat_fit
                    .entry(sample.category.label().to_string())
                    .or_insert(0) += 1;
                let ps_entry = source_payload_sum
                    .entry(sample.source.clone())
                    .or_insert((0, 0));
                ps_entry.0 += 1;
                ps_entry.1 += *payload_len;
                let pc_entry = cat_payload_sum
                    .entry(sample.category.label().to_string())
                    .or_insert((0, 0));
                pc_entry.0 += 1;
                pc_entry.1 += *payload_len;

                if mode == "dictionary" {
                    dict_count += 1;
                    *source_dict.entry(sample.source.clone()).or_insert(0) += 1;
                    *cat_dict
                        .entry(sample.category.label().to_string())
                        .or_insert(0) += 1;
                    if let Some(name) = dict_name {
                        let entry = dict_payload_sums.entry(name.clone()).or_insert((0, 0));
                        entry.0 += 1;
                        entry.1 += *payload_len;
                        *category_by_dict_total
                            .entry(name.clone())
                            .or_default()
                            .entry(sample.category.label().to_string())
                            .or_insert(0) += 1;
                    }
                } else {
                    *fallback_by_mode.entry(mode.clone()).or_insert(0) += 1;
                    *source_modes
                        .entry(sample.source.clone())
                        .or_default()
                        .entry(mode.clone())
                        .or_insert(0) += 1;
                    *cat_modes
                        .entry(sample.category.label().to_string())
                        .or_default()
                        .entry(mode.clone())
                        .or_insert(0) += 1;
                    non_dict.push(NonDictEntry {
                        text: sample.text.clone(),
                        category: sample.category.label().to_string(),
                        source: sample.source.clone(),
                        mode: mode.clone(),
                        payload_len: *payload_len,
                        warnings: warnings.clone(),
                    });
                    worst_fitting.push(WorstFitEntry {
                        text: sample.text.clone(),
                        category: sample.category.label().to_string(),
                        source: sample.source.clone(),
                        mode: mode.clone(),
                        dict_name: dict_name.clone(),
                        payload_len: *payload_len,
                    });
                    let tokens = tokenize_lower(&sample.text);
                    for t in &tokens {
                        if t.len() >= 3 {
                            *missing_word_counts.entry(t.clone()).or_insert(0) += 1;
                            *aggr
                                .by_source_word
                                .entry(sample.source.clone())
                                .or_default()
                                .entry(t.clone())
                                .or_insert(0) += 1;
                            *aggr
                                .by_category_word
                                .entry(sample.category.label().to_string())
                                .or_default()
                                .entry(t.clone())
                                .or_insert(0) += 1;
                            word_to_sources
                                .entry(t.clone())
                                .or_default()
                                .insert(sample.source.clone());
                            word_to_categories
                                .entry(t.clone())
                                .or_default()
                                .insert(sample.category.label().to_string());
                            if is_synthetic_sample(sample) {
                                *word_to_synthetic_count.entry(t.clone()).or_insert(0) += 1;
                            } else {
                                *word_to_nonsynth_count.entry(t.clone()).or_insert(0) += 1;
                            }
                        }
                    }
                    for win in tokens.windows(2) {
                        let phrase = format!("{} {}", win[0], win[1]);
                        *missing_bigram_counts.entry(phrase).or_insert(0) += 1;
                    }
                    for win in tokens.windows(3) {
                        let phrase = format!("{} {} {}", win[0], win[1], win[2]);
                        *missing_trigram_counts.entry(phrase).or_insert(0) += 1;
                    }
                    for win in tokens.windows(4) {
                        let phrase = format!("{} {} {} {}", win[0], win[1], win[2], win[3]);
                        *missing_fourgram_counts.entry(phrase).or_insert(0) += 1;
                    }
                }
                sample_reports.push(SampleReport {
                    text: sample.text.clone(),
                    category: sample.category.label().to_string(),
                    source: sample.source.clone(),
                    outcome: "fit".to_string(),
                    mode: Some(mode.clone()),
                    dict_name: dict_name.clone(),
                    payload_len: Some(*payload_len),
                    roundtrip_text: Some(roundtrip_text.clone()),
                    roundtrip_ok: Some(*roundtrip_ok),
                    warnings: Some(warnings.clone()),
                    failure_reason: None,
                });
            }
            Outcome::Fail { reason } => {
                fail_count += 1;
                *source_fail.entry(sample.source.clone()).or_insert(0) += 1;
                *cat_fail
                    .entry(sample.category.label().to_string())
                    .or_insert(0) += 1;
                failures.push(FailureEntry {
                    text: sample.text.clone(),
                    category: sample.category.label().to_string(),
                    source: sample.source.clone(),
                    raw_len: sample.text.len(),
                    reason: reason.clone(),
                });
                sample_reports.push(SampleReport {
                    text: sample.text.clone(),
                    category: sample.category.label().to_string(),
                    source: sample.source.clone(),
                    outcome: "fail".to_string(),
                    mode: None,
                    dict_name: None,
                    payload_len: None,
                    roundtrip_text: None,
                    roundtrip_ok: None,
                    warnings: None,
                    failure_reason: Some(reason.clone()),
                });
            }
        }
    }

    payload_lengths.sort_unstable();
    let avg = if !payload_lengths.is_empty() {
        payload_lengths.iter().sum::<usize>() as f64 / payload_lengths.len() as f64
    } else {
        0.0
    };
    let mut fallback_pct: BTreeMap<String, f64> = BTreeMap::new();
    for (k, v) in &fallback_by_mode {
        fallback_pct.insert(k.clone(), pct(*v, total));
    }
    let summary = Summary {
        total_samples: total,
        fit_count,
        fail_count,
        fit_pct: pct(fit_count, total),
        fail_pct: pct(fail_count, total),
        dict_mode_count: dict_count,
        dict_mode_pct: pct(dict_count, total),
        fallback_by_mode: fallback_by_mode.clone(),
        avg_payload_len: avg,
        p50_payload_len: percentile(&payload_lengths, 0.50),
        p90_payload_len: percentile(&payload_lengths, 0.90),
        p95_payload_len: percentile(&payload_lengths, 0.95),
        min_payload_len: *payload_lengths.first().unwrap_or(&0),
        max_payload_len: *payload_lengths.last().unwrap_or(&0),
        near_limit_count,
        near_limit_pct: pct(near_limit_count, total),
    };
    let coverage_scorecard = CoverageScorecard {
        fit_pct: summary.fit_pct,
        dict_pct: summary.dict_mode_pct,
        fallback_pct,
        failure_pct: summary.fail_pct,
        avg_payload_len: summary.avg_payload_len,
        p50_payload_len: summary.p50_payload_len,
        p90_payload_len: summary.p90_payload_len,
        p95_payload_len: summary.p95_payload_len,
        near_limit_count: summary.near_limit_count,
        near_limit_pct: summary.near_limit_pct,
    };

    let mut distribution: Vec<DictStats> = Vec::new();
    for (name, _dict) in DICTIONARIES.iter() {
        let key = name.to_string();
        let (count, sum) = dict_payload_sums.get(&key).copied().unwrap_or((0, 0));
        let avg_payload = if count > 0 {
            sum as f64 / count as f64
        } else {
            0.0
        };
        let mut sample_categories: BTreeMap<String, usize> = BTreeMap::new();
        if let Some(map) = category_by_dict_total.get(&key) {
            for (cat, n) in map {
                sample_categories.insert(cat.clone(), *n);
            }
        }
        distribution.push(DictStats {
            name: key.clone(),
            count,
            pct_of_dict: pct(count, dict_count),
            avg_payload_len: avg_payload,
            sample_categories,
        });
    }

    let identity = table_identity();
    let table_identity_report = TableIdentityReport {
        version_marker: identity.version_marker.clone(),
        dictionary_count: identity.dictionary_count,
        entry_counts: identity.entry_counts.clone(),
        suffix_count: identity.suffix_count,
        fingerprint_sha256: identity.fingerprint_sha256.clone(),
    };

    worst_fitting.sort_by(|a, b| b.payload_len.cmp(&a.payload_len).then(a.text.cmp(&b.text)));
    worst_fitting.truncate(100);

    // Source summaries
    let mut source_summaries: Vec<SourceSummary> = source_total
        .iter()
        .map(|(src, (n, _cat))| {
            let total_n = *n;
            let fit = *source_fit.get(src).unwrap_or(&0);
            let fail = *source_fail.get(src).unwrap_or(&0);
            let dict = *source_dict.get(src).unwrap_or(&0);
            let modes = source_modes.get(src).cloned().unwrap_or_default();
            let (pn, ps) = source_payload_sum.get(src).copied().unwrap_or((0, 0));
            let avg = if pn > 0 { ps as f64 / pn as f64 } else { 0.0 };
            SourceSummary {
                source: src.clone(),
                category: _cat.label().to_string(),
                total: total_n,
                fit,
                fail,
                fit_pct: pct(fit, total_n),
                dict,
                dict_pct: pct(dict, total_n),
                fallback_by_mode: modes,
                avg_payload_len: avg,
            }
        })
        .collect();
    source_summaries.sort_by(|a, b| b.total.cmp(&a.total).then(a.source.cmp(&b.source)));

    // Category summaries
    let mut category_summaries: Vec<CategorySummary> = cat_total
        .iter()
        .map(|(cat, n)| {
            let total_n = *n;
            let fit = *cat_fit.get(cat).unwrap_or(&0);
            let fail = *cat_fail.get(cat).unwrap_or(&0);
            let dict = *cat_dict.get(cat).unwrap_or(&0);
            let modes = cat_modes.get(cat).cloned().unwrap_or_default();
            let (pn, ps) = cat_payload_sum.get(cat).copied().unwrap_or((0, 0));
            let avg = if pn > 0 { ps as f64 / pn as f64 } else { 0.0 };
            CategorySummary {
                category: cat.clone(),
                total: total_n,
                fit,
                fail,
                fit_pct: pct(fit, total_n),
                fail_pct: pct(fail, total_n),
                dict,
                dict_pct: pct(dict, total_n),
                fallback_by_mode: modes,
                avg_payload_len: avg,
            }
        })
        .collect();
    category_summaries.sort_by(|a, b| b.total.cmp(&a.total).then(a.category.cmp(&b.category)));

    // Miss-by-category / miss-by-source
    let mut miss_by_category: Vec<MissByCategory> = aggr
        .by_category_mode
        .iter()
        .map(|(cat, modes)| {
            let mut top_words: Vec<(String, usize)> = aggr
                .by_category_word
                .get(cat)
                .map(|m| m.iter().map(|(k, v)| (k.clone(), *v)).collect())
                .unwrap_or_default();
            top_words.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            top_words.truncate(15);
            MissByCategory {
                category: cat.clone(),
                count: modes.values().sum(),
                fallback_modes: modes.clone(),
                top_missing_words: top_words,
            }
        })
        .collect();
    miss_by_category.sort_by(|a, b| b.count.cmp(&a.count).then(a.category.cmp(&b.category)));

    let mut miss_by_source: Vec<MissBySource> = aggr
        .by_source_mode
        .iter()
        .map(|(src, modes)| {
            let mut top_words: Vec<(String, usize)> = aggr
                .by_source_word
                .get(src)
                .map(|m| m.iter().map(|(k, v)| (k.clone(), *v)).collect())
                .unwrap_or_default();
            top_words.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            top_words.truncate(15);
            MissBySource {
                source: src.clone(),
                count: modes.values().sum(),
                fallback_modes: modes.clone(),
                top_missing_words: top_words,
            }
        })
        .collect();
    miss_by_source.sort_by(|a, b| b.count.cmp(&a.count).then(a.source.cmp(&b.source)));

    // N-gram candidates with cross-source / cross-category flags.
    let mut ngram_candidates: Vec<NgramCandidate> = Vec::new();
    let min_word_count = 5usize;
    let min_phrase_count = 3usize;

    let mut words: Vec<(String, usize)> = missing_word_counts
        .iter()
        .filter(|(_, c)| **c >= min_word_count)
        .map(|(k, v)| (k.clone(), *v))
        .collect();
    words.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    for (surface, count) in words.into_iter().take(60) {
        let source_count = word_to_sources.get(&surface).map(|s| s.len()).unwrap_or(0);
        let category_count = word_to_categories
            .get(&surface)
            .map(|s| s.len())
            .unwrap_or(0);
        let only_synthetic = *word_to_nonsynth_count.get(&surface).unwrap_or(&0) == 0;
        let likely_id = is_likely_id_or_noise(&surface);
        let likely_too_long = false;
        ngram_candidates.push(NgramCandidate {
            suggested_dict: classify_word(&surface),
            kind: "word".to_string(),
            surface,
            count,
            source_count,
            category_count,
            only_synthetic,
            likely_id_or_noise: likely_id,
            likely_too_long,
        });
    }

    let mut bigrams: Vec<(String, usize)> = missing_bigram_counts
        .into_iter()
        .filter(|(_, c)| *c >= min_phrase_count)
        .collect();
    bigrams.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    for (surface, count) in bigrams.into_iter().take(40) {
        let tokens: Vec<&str> = surface.split_whitespace().collect();
        let likely_id = tokens.iter().any(|t| is_likely_id_or_noise(t));
        ngram_candidates.push(NgramCandidate {
            suggested_dict: classify_phrase(&surface),
            kind: "bigram".to_string(),
            surface,
            count,
            source_count: 0,
            category_count: 0,
            only_synthetic: false,
            likely_id_or_noise: likely_id,
            likely_too_long: false,
        });
    }

    let mut trigrams: Vec<(String, usize)> = missing_trigram_counts
        .into_iter()
        .filter(|(_, c)| *c >= min_phrase_count)
        .collect();
    trigrams.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    for (surface, count) in trigrams.into_iter().take(30) {
        let tokens: Vec<&str> = surface.split_whitespace().collect();
        let likely_id = tokens.iter().any(|t| is_likely_id_or_noise(t));
        ngram_candidates.push(NgramCandidate {
            suggested_dict: classify_phrase(&surface),
            kind: "trigram".to_string(),
            surface,
            count,
            source_count: 0,
            category_count: 0,
            only_synthetic: false,
            likely_id_or_noise: likely_id,
            likely_too_long: false,
        });
    }

    let mut fourgrams: Vec<(String, usize)> = missing_fourgram_counts
        .into_iter()
        .filter(|(_, c)| *c >= min_phrase_count)
        .collect();
    fourgrams.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    for (surface, count) in fourgrams.into_iter().take(20) {
        let tokens: Vec<&str> = surface.split_whitespace().collect();
        let likely_id = tokens.iter().any(|t| is_likely_id_or_noise(t));
        ngram_candidates.push(NgramCandidate {
            suggested_dict: classify_phrase(&surface),
            kind: "fourgram".to_string(),
            surface,
            count,
            source_count: 0,
            category_count: 0,
            only_synthetic: false,
            likely_id_or_noise: likely_id,
            likely_too_long: false,
        });
    }

    ngram_candidates.sort_by(|a, b| {
        a.kind
            .cmp(&b.kind)
            .then(b.count.cmp(&a.count))
            .then(a.surface.cmp(&b.surface))
    });

    // "Do not tune yet": candidates that are likely ID/noise OR are only
    // seen in synthetic sources.
    let do_not_tune_yet: Vec<NgramCandidate> = ngram_candidates
        .iter()
        .filter(|c| c.likely_id_or_noise || c.only_synthetic)
        .cloned()
        .collect();

    // Cross-dictionary leakage.
    let mut dictionary_words: HashMap<String, String> = HashMap::new();
    for (name, dict) in DICTIONARIES.iter() {
        for entry in dict.iter() {
            let e = entry.trim();
            if e.is_empty() || e == " " {
                continue;
            }
            let key = e.to_lowercase();
            dictionary_words
                .entry(key)
                .or_insert_with(|| name.to_string());
        }
    }
    let mut cross_leakage: Vec<CrossLeak> = Vec::new();
    for c in &ngram_candidates {
        if c.kind != "word" {
            continue;
        }
        if c.surface.contains(' ') {
            continue;
        }
        if let Some(current_dict) = dictionary_words.get(&c.surface).cloned() {
            if c.count >= 5 {
                cross_leakage.push(CrossLeak {
                    surface: c.surface.clone(),
                    count: c.count,
                    current_dict,
                });
            }
        }
    }
    cross_leakage.sort_by(|a, b| b.count.cmp(&a.count).then(a.surface.cmp(&b.surface)));
    cross_leakage.truncate(40);

    // Low-value tokens.
    let mut single_word_keys: Vec<(String, String)> = Vec::new();
    for (name, dict) in DICTIONARIES.iter() {
        for entry in dict.iter() {
            let e = entry.trim();
            if e.is_empty() || e.contains(' ') {
                continue;
            }
            let key = e.to_lowercase();
            if !key.chars().any(|c| c.is_alphabetic()) {
                continue;
            }
            single_word_keys.push((name.to_string(), key));
        }
    }
    let mut token_corpus_hits: HashMap<(String, String), usize> = HashMap::new();
    for sample in samples {
        let lower = normalize_text(&sample.text);
        for (name, key) in &single_word_keys {
            if lower.contains(key.as_str()) {
                *token_corpus_hits
                    .entry((name.clone(), key.clone()))
                    .or_insert(0) += 1;
            }
        }
    }
    let mut low_value_tokens: Vec<LowValueToken> = Vec::new();
    for (name, key) in &single_word_keys {
        let hits = token_corpus_hits
            .get(&(name.clone(), key.clone()))
            .copied()
            .unwrap_or(0);
        if hits == 0 {
            low_value_tokens.push(LowValueToken {
                surface: key.clone(),
                dict: name.clone(),
                corpus_occurrences: hits,
            });
        }
    }
    low_value_tokens.sort_by(|a, b| a.dict.cmp(&b.dict).then(a.surface.cmp(&b.surface)));
    low_value_tokens.truncate(80);

    let regression_watchlist = build_watchlist(samples, outcomes);

    // Next-data suggestions: pick the categories with the lowest dict%
    // and the highest fail% as needing more real samples.
    let mut next_data_suggestions: Vec<String> = Vec::new();
    let mut cat_need: Vec<(String, f64, f64, usize)> = category_summaries
        .iter()
        .map(|c| {
            let need_score = c.fail_pct + (100.0 - c.dict_pct) * 0.5;
            (
                c.category.clone(),
                c.dict_pct,
                c.fail_pct,
                need_score as usize,
            )
        })
        .collect();
    cat_need.sort_by(|a, b| b.3.cmp(&a.3).then(a.0.cmp(&b.0)));
    for (cat, dict_pct, fail_pct, _) in cat_need.iter().take(4) {
        next_data_suggestions.push(format!(
            "collect more real-world samples for category `{}` (current dict {:.1}%, fail {:.1}%)",
            cat, dict_pct, fail_pct
        ));
    }
    next_data_suggestions.push(
        "add samples that include the long-tail ops phrases (passed on, merged to, for sprint, etc.) from real workflows"
            .to_string(),
    );
    next_data_suggestions.push(
        "add real support / sales / shipping snippets from a public source with clear license"
            .to_string(),
    );

    AuditReport {
        generated_at_unix: now_unix(),
        generated_at_iso: now_iso8601(),
        run_label: run_label.clone(),
        corpus_sources: CorpusSources {
            built_in_samples: sources.built_in_samples,
            per_file: sources.per_file.clone(),
            duplicates_removed: sources.duplicates_removed,
            total_unique: total,
            corpus_dirs: sources.corpus_dirs.clone(),
            input_files: sources.input_files.clone(),
            no_built_in: sources.no_built_in,
        },
        table_identity: table_identity_report,
        summary,
        coverage_scorecard,
        dictionary_distribution: distribution,
        source_summaries,
        category_summaries,
        worst_fitting,
        failures,
        non_dictionary: non_dict,
        miss_by_category,
        miss_by_source,
        ngram_candidates,
        do_not_tune_yet,
        cross_dictionary_leakage: cross_leakage,
        low_value_tokens,
        regression_watchlist,
        next_data_suggestions,
        samples: sample_reports,
    }
}

fn build_watchlist(samples: &[Sample], outcomes: &[Outcome]) -> WatchlistReport {
    let mut should_stay_in_dict_mode: Vec<String> = Vec::new();
    let mut should_fit_even_if_not_dict: Vec<String> = Vec::new();

    let per_category_target = 25 / 7 + 1;
    for cat in &[
        Category::CasualChat,
        Category::BusinessPayment,
        Category::OperationsSupport,
        Category::AssetHolder,
        Category::LogisticsTrace,
        Category::CryptoHemp0x,
        Category::EdgeCases,
    ] {
        let mut pushed_dict = 0;
        let mut pushed_fit = 0;
        for (s, o) in samples.iter().zip(outcomes.iter()) {
            if &s.category != cat {
                continue;
            }
            if let Outcome::Fit { mode, .. } = o {
                if mode == "dictionary" && pushed_dict < per_category_target {
                    should_stay_in_dict_mode.push(s.text.clone());
                    pushed_dict += 1;
                } else if mode != "dictionary" && pushed_fit < per_category_target {
                    should_fit_even_if_not_dict.push(s.text.clone());
                    pushed_fit += 1;
                }
            }
            if pushed_dict >= per_category_target && pushed_fit >= per_category_target {
                break;
            }
        }
    }

    should_stay_in_dict_mode.truncate(25);
    should_fit_even_if_not_dict.truncate(25);
    WatchlistReport {
        should_stay_in_dict_mode,
        should_fit_even_if_not_dict,
    }
}

fn write_markdown_report(path: &Path, report: &AuditReport) -> std::io::Result<()> {
    let mut f = fs::File::create(path)?;
    writeln!(f, "# Short Message Corpus Coverage Audit")?;
    writeln!(f)?;
    writeln!(
        f,
        "- Generated: {} (unix={})",
        report.generated_at_iso, report.generated_at_unix
    )?;
    if let Some(label) = &report.run_label {
        writeln!(f, "- Run label: `{}`", label)?;
    }
    writeln!(f)?;

    writeln!(f, "## Corpus Sources")?;
    writeln!(f)?;
    let s = &report.corpus_sources;
    writeln!(f, "- Built-in samples: {}", s.built_in_samples)?;
    writeln!(f, "- `--no-built-in` set: {}", s.no_built_in)?;
    if !s.corpus_dirs.is_empty() {
        writeln!(f, "- Corpus directories:")?;
        for dir in &s.corpus_dirs {
            writeln!(f, "  - `{}`", dir)?;
        }
    }
    if !s.input_files.is_empty() {
        writeln!(f, "- Individual input files:")?;
        for file in &s.input_files {
            writeln!(f, "  - `{}`", file)?;
        }
    }
    writeln!(f, "- Per-source sample counts (after dedup):")?;
    for (name, count) in &s.per_file {
        writeln!(f, "  - `{}`: {}", name, count)?;
    }
    writeln!(f, "- Duplicates removed: {}", s.duplicates_removed)?;
    writeln!(f, "- Total unique samples run: {}", s.total_unique)?;
    writeln!(f)?;

    writeln!(f, "## Table Identity")?;
    writeln!(f)?;
    writeln!(
        f,
        "- Version marker: `{}`",
        report.table_identity.version_marker
    )?;
    writeln!(
        f,
        "- Dictionary count: {}",
        report.table_identity.dictionary_count
    )?;
    writeln!(f, "- Suffix count: {}", report.table_identity.suffix_count)?;
    writeln!(
        f,
        "- Fingerprint (SHA-256): `{}`",
        report.table_identity.fingerprint_sha256
    )?;
    writeln!(f, "- Entry counts per dictionary:")?;
    for (name, count) in &report.table_identity.entry_counts {
        writeln!(f, "  - `{}`: {}", name, count)?;
    }
    writeln!(f)?;

    writeln!(f, "## Coverage Scorecard")?;
    writeln!(f)?;
    let c = &report.coverage_scorecard;
    writeln!(f, "| Metric | Value |")?;
    writeln!(f, "|---|---|")?;
    writeln!(f, "| Fit % | {:.2}% |", c.fit_pct)?;
    writeln!(f, "| Dictionary mode % | {:.2}% |", c.dict_pct)?;
    writeln!(f, "| Failure % | {:.2}% |", c.failure_pct)?;
    writeln!(f, "| Avg payload len | {:.2} |", c.avg_payload_len)?;
    writeln!(f, "| p50 payload len | {} |", c.p50_payload_len)?;
    writeln!(f, "| p90 payload len | {} |", c.p90_payload_len)?;
    writeln!(f, "| p95 payload len | {} |", c.p95_payload_len)?;
    writeln!(
        f,
        "| Near-limit (>=24B) | {} ({:.2}%) |",
        c.near_limit_count, c.near_limit_pct
    )?;
    if !c.fallback_pct.is_empty() {
        let mut parts: Vec<String> = c
            .fallback_pct
            .iter()
            .map(|(k, v)| format!("{}: {:.2}%", k, v))
            .collect();
        parts.sort();
        writeln!(f, "| Fallback mix | {} |", parts.join(", "))?;
    }
    writeln!(f)?;

    writeln!(f, "## Dictionary Distribution")?;
    writeln!(f)?;
    writeln!(
        f,
        "| Dict | Count | % of dict mode | Avg payload | Top categories |"
    )?;
    writeln!(f, "|---|---|---|---|---|")?;
    for d in &report.dictionary_distribution {
        let top = if d.sample_categories.is_empty() {
            "-".to_string()
        } else {
            let mut v: Vec<(&String, &usize)> = d.sample_categories.iter().collect();
            v.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
            v.into_iter()
                .take(3)
                .map(|(k, n)| format!("{}:{}", k, n))
                .collect::<Vec<_>>()
                .join(", ")
        };
        writeln!(
            f,
            "| {} | {} | {:.2}% | {:.2} | {} |",
            d.name, d.count, d.pct_of_dict, d.avg_payload_len, top
        )?;
    }
    writeln!(f)?;

    writeln!(f, "## Per-Source Summaries")?;
    writeln!(f)?;
    writeln!(
        f,
        "| source | category | total | fit | fail | dict | avg payload | fallback mix |"
    )?;
    writeln!(f, "|---|---|---|---|---|---|---|---|")?;
    for s in &report.source_summaries {
        let mix: Vec<String> = s
            .fallback_by_mode
            .iter()
            .map(|(k, v)| format!("{}:{}", k, v))
            .collect();
        writeln!(
            f,
            "| {} | {} | {} | {} | {} | {} | {:.2} | {} |",
            s.source,
            s.category,
            s.total,
            s.fit,
            s.fail,
            s.dict,
            s.avg_payload_len,
            mix.join(", ")
        )?;
    }
    writeln!(f)?;

    writeln!(f, "## Per-Category Summaries")?;
    writeln!(f)?;
    writeln!(
        f,
        "| category | total | fit | fail | dict | avg payload | fallback mix |"
    )?;
    writeln!(f, "|---|---|---|---|---|---|---|")?;
    for c in &report.category_summaries {
        let mix: Vec<String> = c
            .fallback_by_mode
            .iter()
            .map(|(k, v)| format!("{}:{}", k, v))
            .collect();
        writeln!(
            f,
            "| {} | {} | {} | {} | {} | {:.2} | {} |",
            c.category,
            c.total,
            c.fit,
            c.fail,
            c.dict,
            c.avg_payload_len,
            mix.join(", ")
        )?;
    }
    writeln!(f)?;

    writeln!(f, "## Worst Fitting Messages (top 100 by payload length)")?;
    writeln!(f)?;
    writeln!(f, "| text | category | source | mode | dict | payload |")?;
    writeln!(f, "|---|---|---|---|---|---|")?;
    for e in report.worst_fitting.iter().take(100) {
        let dict = e.dict_name.clone().unwrap_or_else(|| "-".to_string());
        writeln!(
            f,
            "| `{}` | {} | {} | {} | {} | {} |",
            e.text.replace('|', "\\|"),
            e.category,
            e.source,
            e.mode,
            dict,
            e.payload_len
        )?;
    }
    writeln!(f)?;

    writeln!(f, "## Dictionary Miss Analysis")?;
    writeln!(f)?;
    writeln!(
        f,
        "Total non-dict fit messages: {}",
        report.non_dictionary.len()
    )?;
    writeln!(f)?;
    let mut by_mode: BTreeMap<String, Vec<&NonDictEntry>> = BTreeMap::new();
    for e in &report.non_dictionary {
        by_mode.entry(e.mode.clone()).or_default().push(e);
    }
    for (mode, entries) in &by_mode {
        writeln!(f, "### Mode: `{}` ({} samples)", mode, entries.len())?;
        writeln!(f)?;
        writeln!(f, "| text | category | source | payload | warnings |")?;
        writeln!(f, "|---|---|---|---|---|")?;
        for e in entries.iter().take(40) {
            let w = if e.warnings.is_empty() {
                "-".to_string()
            } else {
                e.warnings.join("; ")
            };
            writeln!(
                f,
                "| `{}` | {} | {} | {} | {} |",
                e.text.replace('|', "\\|"),
                e.category,
                e.source,
                e.payload_len,
                w.replace('|', "\\|")
            )?;
        }
        if entries.len() > 40 {
            writeln!(f, "| _...{} more..._ | | | | |", entries.len() - 40)?;
        }
        writeln!(f)?;
    }

    writeln!(f, "### Miss by Category")?;
    writeln!(f)?;
    writeln!(f, "| category | count | fallback mix | top missing words |")?;
    writeln!(f, "|---|---|---|---|")?;
    for m in &report.miss_by_category {
        let mix: Vec<String> = m
            .fallback_modes
            .iter()
            .map(|(k, v)| format!("{}:{}", k, v))
            .collect();
        let words: Vec<String> = m
            .top_missing_words
            .iter()
            .take(5)
            .map(|(k, v)| format!("{}:{}", k, v))
            .collect();
        writeln!(
            f,
            "| {} | {} | {} | {} |",
            m.category,
            m.count,
            mix.join(", "),
            words.join(", ")
        )?;
    }
    writeln!(f)?;

    writeln!(f, "### Miss by Source")?;
    writeln!(f)?;
    writeln!(f, "| source | count | fallback mix | top missing words |")?;
    writeln!(f, "|---|---|---|---|")?;
    for m in &report.miss_by_source {
        let mix: Vec<String> = m
            .fallback_modes
            .iter()
            .map(|(k, v)| format!("{}:{}", k, v))
            .collect();
        let words: Vec<String> = m
            .top_missing_words
            .iter()
            .take(5)
            .map(|(k, v)| format!("{}:{}", k, v))
            .collect();
        writeln!(
            f,
            "| {} | {} | {} | {} |",
            m.source,
            m.count,
            mix.join(", "),
            words.join(", ")
        )?;
    }
    writeln!(f)?;

    writeln!(f, "### N-gram Tuning Candidates")?;
    writeln!(f)?;
    writeln!(f, "Flags: `only_synthetic` = seen only in synthetic sources; `likely_id` = looks like an ID, hex, or number; `repeats across sources` is shown as `source_count`.")?;
    writeln!(f)?;
    writeln!(
        f,
        "| surface | count | kind | sources | cats | suggested dict | flags |"
    )?;
    writeln!(f, "|---|---|---|---|---|---|---|")?;
    for c in &report.ngram_candidates {
        let mut flags: Vec<String> = Vec::new();
        if c.only_synthetic {
            flags.push("only_synthetic".to_string());
        }
        if c.likely_id_or_noise {
            flags.push("likely_id".to_string());
        }
        if c.likely_too_long {
            flags.push("too_long".to_string());
        }
        if c.kind == "word" && c.source_count >= 2 {
            flags.push(format!("repeats_in_{}_sources", c.source_count));
        }
        if c.kind == "word" && c.category_count >= 2 {
            flags.push(format!("repeats_in_{}_categories", c.category_count));
        }
        writeln!(
            f,
            "| `{}` | {} | {} | {} | {} | {} | {} |",
            c.surface.replace('|', "\\|"),
            c.count,
            c.kind,
            c.source_count,
            c.category_count,
            c.suggested_dict,
            flags.join(", ")
        )?;
    }
    writeln!(f)?;

    writeln!(f, "### Do Not Tune Yet")?;
    writeln!(f)?;
    writeln!(
        f,
        "Candidates that look like IDs/noise or were seen only in synthetic sources."
    )?;
    writeln!(f)?;
    writeln!(f, "| surface | count | kind | reason |")?;
    writeln!(f, "|---|---|---|---|")?;
    for c in &report.do_not_tune_yet {
        let mut reasons: Vec<String> = Vec::new();
        if c.likely_id_or_noise {
            reasons.push("likely_id".to_string());
        }
        if c.only_synthetic {
            reasons.push("only_synthetic".to_string());
        }
        writeln!(
            f,
            "| `{}` | {} | {} | {} |",
            c.surface.replace('|', "\\|"),
            c.count,
            c.kind,
            reasons.join(", ")
        )?;
    }
    writeln!(f)?;

    writeln!(f, "### Cross-Dictionary Leakage")?;
    writeln!(f)?;
    writeln!(
        f,
        "Words that appear frequently in non-dictionary messages but already exist in some dictionary."
    )?;
    writeln!(f)?;
    writeln!(f, "| surface | count | current dict |")?;
    writeln!(f, "|---|---|---|")?;
    for c in &report.cross_dictionary_leakage {
        writeln!(
            f,
            "| `{}` | {} | {} |",
            c.surface.replace('|', "\\|"),
            c.count,
            c.current_dict
        )?;
    }
    writeln!(f)?;

    writeln!(f, "## Failures")?;
    writeln!(f)?;
    writeln!(f, "Total failures: {}", report.failures.len())?;
    writeln!(f)?;
    if !report.failures.is_empty() {
        writeln!(f, "### Top failures (first 60)")?;
        writeln!(f, "| text | category | source | raw_len | reason |")?;
        writeln!(f, "|---|---|---|---|---|")?;
        for e in report.failures.iter().take(60) {
            writeln!(
                f,
                "| `{}` | {} | {} | {} | {} |",
                e.text.replace('|', "\\|"),
                e.category,
                e.source,
                e.raw_len,
                e.reason.replace('|', "\\|")
            )?;
        }
        if report.failures.len() > 60 {
            writeln!(f, "| _...{} more..._ | | | | |", report.failures.len() - 60)?;
        }
    }
    writeln!(f)?;

    writeln!(f, "## Candidate Replace Guidance")?;
    writeln!(f)?;
    writeln!(f, "### High-value additions")?;
    writeln!(f, "See the `N-gram Tuning Candidates` table above. Prefer entries that are not `only_synthetic` and not `likely_id`, and that repeat across multiple sources or categories.")?;
    writeln!(f)?;
    writeln!(f, "### Low-value existing tokens")?;
    writeln!(f)?;
    writeln!(
        f,
        "Single-word dictionary entries that did not appear (case-insensitive substring) in any sample."
    )?;
    writeln!(f)?;
    writeln!(f, "| surface | dict | corpus_occurrences |")?;
    writeln!(f, "|---|---|---|")?;
    for c in &report.low_value_tokens {
        writeln!(
            f,
            "| `{}` | {} | {} |",
            c.surface.replace('|', "\\|"),
            c.dict,
            c.corpus_occurrences
        )?;
    }
    writeln!(f)?;

    writeln!(f, "## Next Data To Collect")?;
    writeln!(f)?;
    for s in &report.next_data_suggestions {
        writeln!(f, "- {}", s)?;
    }
    writeln!(f)?;

    writeln!(f, "## Regression Watchlist")?;
    writeln!(f)?;
    writeln!(f, "### Should stay in dictionary mode")?;
    for line in &report.regression_watchlist.should_stay_in_dict_mode {
        writeln!(f, "- `{}`", line)?;
    }
    writeln!(f)?;
    writeln!(f, "### Should fit even if not dictionary mode")?;
    for line in &report.regression_watchlist.should_fit_even_if_not_dict {
        writeln!(f, "- `{}`", line)?;
    }
    writeln!(f)?;

    writeln!(f, "## Footnotes")?;
    writeln!(
        f,
        "- HOXSHT table pack version marker: `{}`",
        HOXSHT_VERSION_MARKER
    )?;
    writeln!(
        f,
        "- Suffix tokens considered by stem+suffix matches: {} entries",
        SUFFIXES.len()
    )?;
    writeln!(
        f,
        "- See also: `short-message-corpus-nondict-*.txt`, `short-message-corpus-failures-*.txt`, `regression-watchlist.txt`."
    )?;

    Ok(())
}

fn write_regression_watchlist(path: &Path, report: &AuditReport) -> std::io::Result<()> {
    let mut f = fs::File::create(path)?;
    writeln!(f, "# Regression watchlist")?;
    writeln!(f, "# Generated by short_message_corpus_audit.")?;
    writeln!(f, "#")?;
    if let Some(label) = &report.run_label {
        writeln!(f, "# Run label: {}", label)?;
    }
    writeln!(
        f,
        "# Table pack fingerprint: {}",
        report.table_identity.fingerprint_sha256
    )?;
    writeln!(f, "# Should stay in dictionary mode:")?;
    for line in &report.regression_watchlist.should_stay_in_dict_mode {
        writeln!(f, "{}", line)?;
    }
    writeln!(f, "# Should fit even if not dictionary mode:")?;
    for line in &report.regression_watchlist.should_fit_even_if_not_dict {
        writeln!(f, "{}", line)?;
    }
    Ok(())
}

fn write_nondict_txt(path: &Path, entries: &[NonDictEntry]) -> std::io::Result<()> {
    let mut f = fs::File::create(path)?;
    writeln!(f, "# short-message-corpus-nondict")?;
    writeln!(
        f,
        "# One line per non-dictionary-mode message that fit in 32 bytes."
    )?;
    writeln!(
        f,
        "# Format: <mode>\\t<payload_len>\\t<category>\\t<source>\\t<text>"
    )?;
    for e in entries {
        writeln!(
            f,
            "{}\t{}\t{}\t{}\t{}",
            e.mode, e.payload_len, e.category, e.source, e.text
        )?;
    }
    Ok(())
}

fn write_failures_txt(path: &Path, entries: &[FailureEntry]) -> std::io::Result<()> {
    let mut f = fs::File::create(path)?;
    writeln!(f, "# short-message-corpus-failures")?;
    writeln!(f, "# One line per sample that did not fit in 32 bytes.")?;
    writeln!(
        f,
        "# Format: <raw_len>\\t<category>\\t<source>\\t<reason>\\t<text>"
    )?;
    for e in entries {
        writeln!(
            f,
            "{}\t{}\t{}\t{}\t{}",
            e.raw_len, e.category, e.source, e.reason, e.text
        )?;
    }
    Ok(())
}

fn ensure_dir(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        fs::create_dir_all(path)?;
    }
    Ok(())
}

fn remove_dir_if_exists(path: &Path) -> std::io::Result<()> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    Ok(())
}

fn print_console_summary(report: &AuditReport) {
    let s = &report.summary;
    println!(
        "short-message corpus audit: {} samples, fit={} ({:.2}%), fail={} ({:.2}%), dict={} ({:.2}%)",
        s.total_samples,
        s.fit_count,
        s.fit_pct,
        s.fail_count,
        s.fail_pct,
        s.dict_mode_count,
        s.dict_mode_pct
    );
    println!("fallback modes: {:?}", s.fallback_by_mode);
    println!(
        "payload len avg={:.2} p50={} p90={} p95={} near_limit(>=24B)={} ({:.2}%)",
        s.avg_payload_len,
        s.p50_payload_len,
        s.p90_payload_len,
        s.p95_payload_len,
        s.near_limit_count,
        s.near_limit_pct
    );
    println!(
        "table pack: {} fingerprint={}",
        report.table_identity.version_marker, report.table_identity.fingerprint_sha256
    );
}

struct CliArgs {
    corpus_dirs: Vec<PathBuf>,
    no_built_in: bool,
    out_dir: PathBuf,
    run_label: Option<String>,
    clean_out_dir: bool,
    input_files: Vec<PathBuf>,
}

fn parse_args() -> Result<CliArgs, String> {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut corpus_dirs: Vec<PathBuf> = Vec::new();
    let mut no_built_in = false;
    let mut out_dir = PathBuf::from("untracked/commander-v1.4/short-message-corpus-audit");
    let mut run_label: Option<String> = None;
    let mut clean_out_dir = false;
    let mut input_files: Vec<PathBuf> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        match a.as_str() {
            "--corpus-dir" => {
                let v = args
                    .get(i + 1)
                    .ok_or_else(|| "--corpus-dir requires a PATH".to_string())?;
                corpus_dirs.push(PathBuf::from(v));
                i += 2;
            }
            "--no-built-in" => {
                no_built_in = true;
                i += 1;
            }
            "--out-dir" => {
                let v = args
                    .get(i + 1)
                    .ok_or_else(|| "--out-dir requires a PATH".to_string())?;
                out_dir = PathBuf::from(v);
                i += 2;
            }
            "--run-label" => {
                let v = args
                    .get(i + 1)
                    .ok_or_else(|| "--run-label requires a LABEL".to_string())?;
                run_label = Some(v.clone());
                i += 2;
            }
            "--clean-out-dir" => {
                clean_out_dir = true;
                i += 1;
            }
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            other => {
                if other.starts_with("--") {
                    return Err(format!("unknown flag: {}", other));
                }
                input_files.push(PathBuf::from(other));
                i += 1;
            }
        }
        let _ = a;
    }
    let _ = args;
    Ok(CliArgs {
        corpus_dirs,
        no_built_in,
        out_dir,
        run_label,
        clean_out_dir,
        input_files,
    })
}

fn print_help() {
    println!("short_message_corpus_audit");
    println!();
    println!("USAGE:");
    println!("    short_message_corpus_audit [FLAGS] [FILES...]");
    println!();
    println!("FLAGS:");
    println!("    --corpus-dir PATH     Load every *.txt file in PATH. May be repeated.");
    println!("    --no-built-in         Skip the in-code built-in corpus.");
    println!("    --out-dir PATH        Override the report output directory.");
    println!("    --run-label LABEL     Write reports under <out-dir>/runs/<LABEL>/.");
    println!("    --clean-out-dir       Remove the run output directory before writing.");
    println!("                         Only the run directory is touched; never the whole");
    println!("                         audit folder.");
    println!("    -h, --help            Print this help.");
    println!();
    println!("ARGS:");
    println!("    <file>...             One or more individual sample files.");
}

fn main() {
    let cli = match parse_args() {
        Ok(c) => c,
        Err(err) => {
            eprintln!("error: {}", err);
            std::process::exit(2);
        }
    };

    // Resolve the effective output directory.
    let effective_out_dir = match &cli.run_label {
        Some(label) => cli.out_dir.join("runs").join(label),
        None => cli.out_dir.clone(),
    };

    if cli.clean_out_dir {
        if let Err(err) = remove_dir_if_exists(&effective_out_dir) {
            eprintln!(
                "error: could not clean run output dir {}: {}",
                effective_out_dir.display(),
                err
            );
            std::process::exit(2);
        }
        eprintln!(
            "info: cleaned run output dir {}",
            effective_out_dir.display()
        );
    }

    let mut all: Vec<Sample> = Vec::new();
    let mut per_file: BTreeMap<String, usize> = BTreeMap::new();
    let mut input_files_for_report: Vec<String> = Vec::new();

    if !cli.no_built_in {
        let built_in = collect_built_in();
        *per_file.entry("(built-in)".to_string()).or_insert(0) += built_in.len();
        all.extend(built_in);
    }

    for dir in &cli.corpus_dirs {
        match load_corpus_dir(dir) {
            Ok(samples) => {
                *per_file
                    .entry(format!("(corpus-dir:{})", dir.display()))
                    .or_insert(0) += samples.len();
                all.extend(samples);
            }
            Err(err) => {
                eprintln!("warning: {}", err);
            }
        }
    }

    for path in &cli.input_files {
        let label = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?")
            .to_string();
        match load_extra_file(&path, &label) {
            Ok(samples) => {
                *per_file.entry(format!("(file:{})", label)).or_insert(0) += samples.len();
                input_files_for_report.push(path.display().to_string());
                all.extend(samples);
            }
            Err(err) => {
                eprintln!("warning: {}", err);
            }
        }
    }

    let built_in_count = if cli.no_built_in {
        0
    } else {
        per_file.get("(built-in)").copied().unwrap_or(0)
    };
    let (all, removed) = dedup(all);
    if removed > 0 {
        eprintln!("info: removed {} duplicate samples", removed);
    }
    let mut per_file_post: BTreeMap<String, usize> = BTreeMap::new();
    // Build a reverse map: source label -> display key.
    let mut source_display: HashMap<String, String> = HashMap::new();
    source_display.insert("built-in".to_string(), "(built-in)".to_string());
    for dir in &cli.corpus_dirs {
        let label = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?")
            .to_string();
        let display = format!("(corpus-dir:{})", dir.display());
        source_display.insert(label, display);
    }
    for path in &cli.input_files {
        let label = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?")
            .to_string();
        let display = format!("(file:{})", label);
        source_display.insert(label, display);
    }
    for s in &all {
        let key = source_display
            .get(&s.source)
            .cloned()
            .unwrap_or_else(|| format!("(file:{})", s.source));
        *per_file_post.entry(key).or_insert(0) += 1;
    }

    let outcomes: Vec<Outcome> = all.iter().map(run_sample).collect();
    let sources = CorpusSources {
        built_in_samples: built_in_count,
        per_file: per_file_post,
        duplicates_removed: removed,
        total_unique: all.len(),
        corpus_dirs: cli
            .corpus_dirs
            .iter()
            .map(|p| p.display().to_string())
            .collect(),
        input_files: input_files_for_report,
        no_built_in: cli.no_built_in,
    };
    let report = build_report(&all, &outcomes, &sources, &cli.run_label);

    if let Err(err) = ensure_dir(&effective_out_dir) {
        eprintln!(
            "error: could not create output dir {}: {}",
            effective_out_dir.display(),
            err
        );
        std::process::exit(2);
    }

    let stamp = timestamp_stamp();
    let md_path = effective_out_dir.join(format!("short-message-corpus-audit-{}.md", stamp));
    let json_path = effective_out_dir.join(format!("short-message-corpus-audit-{}.json", stamp));
    let nondict_path =
        effective_out_dir.join(format!("short-message-corpus-nondict-{}.txt", stamp));
    let failures_path =
        effective_out_dir.join(format!("short-message-corpus-failures-{}.txt", stamp));
    let watchlist_path = effective_out_dir.join("regression-watchlist.txt");

    if let Err(err) = write_markdown_report(&md_path, &report) {
        eprintln!("error: failed to write markdown report: {}", err);
        std::process::exit(2);
    }
    if let Err(err) = fs::write(
        &json_path,
        serde_json::to_string_pretty(&report).unwrap_or_else(|e| {
            eprintln!("warn: json serialization failed: {}", e);
            String::from("{}")
        }),
    ) {
        eprintln!("error: failed to write json report: {}", err);
        std::process::exit(2);
    }
    if let Err(err) = write_nondict_txt(&nondict_path, &report.non_dictionary) {
        eprintln!("error: failed to write nondict txt: {}", err);
        std::process::exit(2);
    }
    if let Err(err) = write_failures_txt(&failures_path, &report.failures) {
        eprintln!("error: failed to write failures txt: {}", err);
        std::process::exit(2);
    }
    if let Err(err) = write_regression_watchlist(&watchlist_path, &report) {
        eprintln!("error: failed to write watchlist: {}", err);
        std::process::exit(2);
    }

    print_console_summary(&report);
    if let Some(label) = &report.run_label {
        println!();
        println!("run label: {}", label);
    }
    println!();
    println!("Reports written:");
    println!("  - {}", md_path.display());
    println!("  - {}", json_path.display());
    println!("  - {}", nondict_path.display());
    println!("  - {}", failures_path.display());
    println!("  - {}", watchlist_path.display());
}
