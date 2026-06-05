#![allow(dead_code)]

// short_message_tuning_dry_run
//
// Dev-only scenario runner for short-message dictionary tuning.
//
// This binary does NOT modify the production `short_message_tables.rs`. It
// reads a JSON scenario file, loads the same corpus the audit would load,
// and runs a faithful-enough longest-prefix-match simulation against a
// cloned, in-memory dictionary pack.
//
// Simulation model (approximate but principled):
//   - For each scenario:
//     * Clone the production dict pack.
//     * Apply per-dict replace / add (additions append; replacements swap
//       a specific existing entry).
//     * For every corpus sample:
//       - Lower-case the input (the codec normalizes case).
//       - Greedy left-to-right scan, longest-prefix match across all 8
//         scenario dicts (entries sorted by length desc, then index asc,
//         matching the production `match_order`).
//       - Cost per token: 1 byte (dict mode).
//       - For non-dict fallback, cost is the bit-packing cost of either
//         5bit or 6bit mode (whichever is shorter, after 5bit/6bit
//         coverage check).
//       - A message "fits" when its chosen-mode payload cost <= PAYLOAD_MAX
//         (27 bytes).
//       - A message "is dict" when at least one dict token matched.
//
// The simulation is intentionally simpler than the production encoder in
// three places (all conservative):
//   1. The production encoder also tries stem+suffix combinations and
//      prefers longest consecutive token runs. The dry run does not.
//   2. The production encoder can fall back to ASCII literal runs in
//      dict mode (2-byte header + bytes). The dry run approximates this
//      by treating unmatched characters as "uncovered" (counts as
//      non-dict fallback for that part of the message).
//   3. The production encoder picks the *cheapest* mode (dict, raw, 5bit,
//      6bit) that fits. The dry run uses the same rule.
//
// The point of the dry run is to compare scenarios to the baseline on a
// level playing field and produce per-scenario coverage deltas, top
// improved/regressed messages, and a list of low-value existing tokens to
// consider replacing. The numbers are accurate enough to make tuning
// decisions; the bit-exact hex output is not.
//
// CLI:
//   short_message_tuning_dry_run \
//     --scenarios PATH/TO/scenarios.json \
//     --corpus-dir PATH \
//     [--corpus-dir PATH ...] \
//     [--out-dir PATH] \
//     [--run-label LABEL] \
//     [--clean-out-dir]
//
// Reports are written to <out-dir>/runs/<LABEL>/.

use app_lib::modules::short_message_tables::{
    table_identity, ALPHABET_5BIT, ALPHABET_6BIT, DICTIONARIES, DICT_A, DICT_B, DICT_C, DICT_D,
    DICT_E, DICT_F, DICT_G, DICT_H, HOXSHT_VERSION_MARKER, SUFFIXES,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const PAYLOAD_MAX: usize = 27;

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

#[derive(Deserialize, Debug, Default, Clone)]
struct Meta {
    #[serde(default)]
    source_id: Option<String>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    source_type: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
struct ScenarioEntry {
    dict: String,
    surface: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
struct ScenarioReplace {
    dict: String,
    existing: String,
    replacement: String,
}

#[derive(Deserialize, Debug, Clone)]
struct Scenario {
    name: String,
    description: String,
    #[serde(default)]
    additions: Vec<ScenarioEntry>,
    #[serde(default)]
    replacements: Vec<ScenarioReplace>,
}

#[derive(Deserialize, Debug)]
struct ScenariosFile {
    scenarios: Vec<Scenario>,
}

#[derive(Clone, Debug)]
struct DictPack {
    name: String,
    entries: Vec<String>,
}

#[derive(Clone, Debug)]
struct TablePack {
    dicts: Vec<DictPack>,
}

impl TablePack {
    fn from_production() -> Self {
        let mut dicts = Vec::new();
        for (name, dict) in DICTIONARIES.iter() {
            let entries: Vec<String> = dict.iter().map(|s| s.to_string()).collect();
            dicts.push(DictPack {
                name: name.to_string(),
                entries,
            });
        }
        TablePack { dicts }
    }

    fn apply(&self, scenario: &Scenario) -> Self {
        let mut pack = self.clone();
        for r in &scenario.replacements {
            let dict = pack
                .dicts
                .iter_mut()
                .find(|d| d.name == r.dict)
                .expect("scenario references unknown dict");
            let mut replaced = false;
            for entry in dict.entries.iter_mut() {
                if entry.to_lowercase() == r.existing.to_lowercase() {
                    *entry = r.replacement.clone();
                    replaced = true;
                    break;
                }
            }
            if !replaced {
                eprintln!(
                    "warn: scenario {} replacement of {:?} in {} did not match any existing entry; appending instead",
                    scenario.name, r.existing, r.dict
                );
                if let Some(last) = dict.entries.last_mut() {
                    if last.is_empty() {
                        *last = r.replacement.clone();
                    } else {
                        eprintln!(
                            "warn: dict {} is full (last entry not empty); cannot append",
                            r.dict
                        );
                    }
                }
            }
        }
        for a in &scenario.additions {
            let dict = pack
                .dicts
                .iter_mut()
                .find(|d| d.name == a.dict)
                .expect("scenario references unknown dict");
            // Try to replace an empty entry first.
            let mut appended = false;
            for entry in dict.entries.iter_mut() {
                if entry.is_empty() {
                    *entry = a.surface.clone();
                    appended = true;
                    break;
                }
            }
            if !appended {
                // For the dry run we append beyond the 256-slot limit. The
                // report will note this and call out the entries that would
                // need to be replaced in the real production pack.
                dict.entries.push(a.surface.clone());
            }
        }
        pack
    }

    fn lookup_index(&self, name: &str) -> Option<usize> {
        self.dicts.iter().position(|d| d.name == name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SimMode {
    Dict(String),
    Fallback5bit,
    Fallback6bit,
}

#[derive(Debug, Clone)]
struct SimResult {
    fits: bool,
    mode: SimMode,
    cost: usize,
    matched_tokens: usize,
    matched_chars: usize,
    is_real: bool,
}

fn normalize_lower(s: &str) -> String {
    s.to_lowercase()
}

fn char_in_alphabet(ch: char, alphabet: &[u8]) -> bool {
    alphabet.iter().any(|&b| b as u32 == ch as u32)
}

fn bit_packed_cost(n: usize, bits: usize) -> usize {
    // ceil(n * bits / 8)
    (n * bits + 7) / 8
}

fn simulate(text: &str, pack: &TablePack, sample_is_real: bool) -> SimResult {
    let lower = normalize_lower(text);
    let bytes = lower.as_bytes();
    let chars_count = lower.chars().count();

    // Build the merged match-order across all dicts: (phrase_lower, phrase_len).
    // Sorted by phrase length desc. The codec sorts within a dict the same
    // way. The cross-dict merge mirrors the encoder's greedy left-to-right
    // scan: it tries every entry in length-desc order, so cross-dict
    // ordering is irrelevant for the matched set.
    let mut all_entries: Vec<String> = Vec::new();
    for dict in &pack.dicts {
        for entry in dict.entries.iter() {
            if entry.is_empty() {
                continue;
            }
            let lower_e = entry.to_lowercase();
            all_entries.push(lower_e);
        }
    }
    all_entries.sort_by(|a, b| b.len().cmp(&a.len()).then(a.cmp(b)));

    // Bucket entries by their first byte to make the per-position scan
    // O(bucket_size) instead of O(all_entries).
    use std::collections::HashMap;
    let mut by_first_byte: HashMap<u8, Vec<(String, bool)>> = HashMap::new();
    for e in &all_entries {
        if e.is_empty() {
            continue;
        }
        if e.ends_with(' ') {
            by_first_byte
                .entry(e.as_bytes()[0])
                .or_default()
                .push((e[..e.len() - 1].to_string(), true));
        } else {
            by_first_byte
                .entry(e.as_bytes()[0])
                .or_default()
                .push((e.clone(), false));
        }
    }
    // Sort each bucket by length desc.
    for bucket in by_first_byte.values_mut() {
        bucket.sort_by(|a, b| b.0.len().cmp(&a.0.len()).then(a.0.cmp(&b.0)));
    }

    // Walk the message and decide the cheapest encoding mode.
    // Costs:
    //   - dict mode: 1 byte per matched token + 2-byte literal escape per
    //     unmatched run (header + length byte) + len(unmatched_run) bytes
    //     of payload.
    //   - 5bit mode: 1 byte per 5 bits of the full message; only if every
    //     char is in the 5bit alphabet.
    //   - 6bit mode: 1 byte per 6 bits of the full message; only if every
    //     char is in the 6bit alphabet.
    //   - raw mode: 2 + len bytes (one literal run).
    //
    // The encoder is smarter (stem+suffix combinations, digit/numeric
    // packing), but the cheapest-mode selector is the same.
    let mut dict_cost = 0usize;
    let mut i = 0usize;
    let mut dict_matched_chars = 0usize;
    let mut dict_matched_tokens = 0usize;
    let mut uncovered_run = 0usize;
    while i < bytes.len() {
        let mut best: Option<(usize, bool)> = None;
        if let Some(bucket) = by_first_byte.get(&bytes[i]) {
            for (phrase, had_trailing_space) in bucket {
                if bytes[i..].starts_with(phrase.as_bytes()) {
                    let len = phrase.len();
                    let at_end = i + len == bytes.len();
                    let before_punct = i + len < bytes.len()
                        && matches!(bytes[i + len], b'.' | b'!' | b'?' | b',' | b';' | b':');
                    let is_match = !*had_trailing_space || at_end || before_punct;
                    if is_match && best.map_or(true, |(b, _)| len > b) {
                        best = Some((len, *had_trailing_space));
                    }
                }
            }
        }
        match best {
            Some((len, _had_trailing_space)) => {
                if uncovered_run > 0 {
                    dict_cost += 2 + uncovered_run;
                    uncovered_run = 0;
                }
                dict_cost += 1;
                dict_matched_chars += len;
                dict_matched_tokens += 1;
                i += len;
            }
            None => {
                uncovered_run += 1;
                i += 1;
            }
        }
    }
    if uncovered_run > 0 {
        dict_cost += 2 + uncovered_run;
    }

    let fivebit_chars = lower
        .chars()
        .filter(|c| char_in_alphabet(*c, &ALPHABET_5BIT))
        .count();
    let fivebit_full = fivebit_chars == chars_count;
    let fivebit_cost = if fivebit_full {
        bit_packed_cost(chars_count, 5)
    } else {
        usize::MAX
    };

    let sixbit_chars = lower
        .chars()
        .filter(|c| char_in_alphabet(*c, &ALPHABET_6BIT))
        .count();
    let sixbit_full = sixbit_chars == chars_count;
    let sixbit_cost = if sixbit_full {
        bit_packed_cost(chars_count, 6)
    } else {
        usize::MAX
    };

    // Choose the cheapest cost among (dict, 5bit, 6bit). Skip the explicit
    // "raw" mode because dict mode with all-literal escape = raw mode.
    let candidates: Vec<(SimMode, usize)> = vec![
        (SimMode::Dict("best".to_string()), dict_cost),
        (SimMode::Fallback5bit, fivebit_cost),
        (SimMode::Fallback6bit, sixbit_cost),
    ];
    let mut sorted: Vec<(SimMode, usize)> = candidates.into_iter().collect();
    sorted.sort_by_key(|(_, c)| *c);
    let (best_mode, best_cost) = sorted.into_iter().next().unwrap();

    if best_cost > PAYLOAD_MAX {
        return SimResult {
            fits: false,
            mode: best_mode,
            cost: best_cost,
            matched_tokens: dict_matched_tokens,
            matched_chars: dict_matched_chars,
            is_real: sample_is_real,
        };
    }

    SimResult {
        fits: true,
        mode: best_mode,
        cost: best_cost,
        matched_tokens: dict_matched_tokens,
        matched_chars: dict_matched_chars,
        is_real: sample_is_real,
    }
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

fn read_meta(path: &Path) -> Meta {
    let meta_path = path.with_extension("meta.json");
    if !meta_path.exists() {
        return Meta::default();
    }
    let text = match fs::read_to_string(&meta_path) {
        Ok(t) => t,
        Err(_) => return Meta::default(),
    };
    serde_json::from_str(&text).unwrap_or_default()
}

fn load_extra_file(path: &Path, default_source_label: &str) -> Result<Vec<Sample>, String> {
    let text = fs::read_to_string(path)
        .map_err(|e| format!("failed to read {}: {}", path.display(), e))?;
    let source_label = default_source_label.to_string();
    let category = category_from_source(default_source_label);
    let mut out = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
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

#[derive(Debug, Serialize, Clone)]
struct ScenarioResult {
    name: String,
    description: String,
    additions: Vec<ScenarioEntry>,
    replacements: Vec<ScenarioReplace>,
    fit_count: usize,
    fit_pct: f64,
    dict_count: usize,
    dict_pct: f64,
    fail_count: usize,
    fail_pct: f64,
    fallback_5bit: usize,
    fallback_6bit: usize,
    avg_cost: f64,
    p50_cost: usize,
    p90_cost: usize,
    p95_cost: usize,
    near_limit: usize,
    real_fit: usize,
    real_total: usize,
    real_fit_pct: f64,
    real_dict: usize,
    real_dict_pct: f64,
    real_fail: usize,
    synthetic_fit: usize,
    synthetic_total: usize,
    synthetic_fit_pct: f64,
    deltas_vs_baseline: BTreeMap<String, f64>,
    top_improved: Vec<(String, String)>,
    top_regressed: Vec<(String, String)>,
}

fn percentile_usize(sorted: &[usize], p: f64) -> usize {
    if sorted.is_empty() {
        return 0;
    }
    let rank = (p * (sorted.len() as f64 - 1.0)).round() as usize;
    sorted[rank.min(sorted.len() - 1)]
}

fn run_scenario(
    name: &str,
    description: &str,
    pack: &TablePack,
    samples: &[Sample],
    baseline: &ScenarioResult,
    baseline_per_sample: &[(bool, usize)],
) -> ScenarioResult {
    let mut fit = 0usize;
    let mut fail = 0usize;
    let mut dict = 0usize;
    let mut fb5 = 0usize;
    let mut fb6 = 0usize;
    let mut costs: Vec<usize> = Vec::new();
    let mut near_limit = 0usize;
    let mut real_fit = 0usize;
    let mut real_total = 0usize;
    let mut real_dict = 0usize;
    let mut real_fail = 0usize;
    let mut synth_fit = 0usize;
    let mut synth_total = 0usize;

    let mut improved: Vec<(i64, String, String)> = Vec::new();
    let mut regressed: Vec<(i64, String, String)> = Vec::new();
    for (i, sample) in samples.iter().enumerate() {
        let res = simulate(&sample.text, pack, !is_synthetic_source(&sample.source));
        let is_real = !is_synthetic_source(&sample.source);
        if is_real {
            real_total += 1;
        } else {
            synth_total += 1;
        }
        if res.fits {
            fit += 1;
            if is_real {
                real_fit += 1;
            } else {
                synth_fit += 1;
            }
            costs.push(res.cost);
            if res.cost >= 24 {
                near_limit += 1;
            }
            match res.mode {
                SimMode::Dict(_) => {
                    dict += 1;
                    if is_real {
                        real_dict += 1;
                    }
                }
                SimMode::Fallback5bit => fb5 += 1,
                SimMode::Fallback6bit => fb6 += 1,
            }
        } else {
            fail += 1;
            if is_real {
                real_fail += 1;
            }
        }
        let base_res = &baseline_per_sample[i];
        let base_fits = base_res.0;
        let base_cost = base_res.1;
        let delta = base_cost as i64 - res.cost as i64;
        if base_fits && !res.fits {
            regressed.push((delta.abs(), sample.text.clone(), sample.source.clone()));
        } else if res.fits && !base_fits {
            improved.push((delta.abs(), sample.text.clone(), sample.source.clone()));
        } else if res.fits && base_fits {
            if res.cost + 1 < base_cost {
                improved.push((delta, sample.text.clone(), sample.source.clone()));
            } else if base_cost + 1 < res.cost {
                regressed.push((delta.abs(), sample.text.clone(), sample.source.clone()));
            }
        }
    }
    costs.sort_unstable();
    let total = samples.len();
    let avg = if !costs.is_empty() {
        costs.iter().sum::<usize>() as f64 / costs.len() as f64
    } else {
        0.0
    };

    improved.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    improved.truncate(20);
    regressed.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    regressed.truncate(20);

    let mut deltas: BTreeMap<String, f64> = BTreeMap::new();
    deltas.insert("fit_pct".into(), pct(fit, total) - baseline.fit_pct);
    deltas.insert("dict_pct".into(), pct(dict, total) - baseline.dict_pct);
    deltas.insert("fail_pct".into(), pct(fail, total) - baseline.fail_pct);
    deltas.insert(
        "real_fit_pct".into(),
        pct(real_fit, real_total) - baseline.real_fit_pct,
    );
    deltas.insert(
        "real_dict_pct".into(),
        pct(real_dict, real_total) - baseline.real_dict_pct,
    );
    deltas.insert("avg_cost".into(), avg - baseline.avg_cost);
    deltas.insert(
        "near_limit".into(),
        near_limit as f64 - baseline.near_limit as f64,
    );

    ScenarioResult {
        name: name.to_string(),
        description: description.to_string(),
        additions: vec![],
        replacements: vec![],
        fit_count: fit,
        fit_pct: pct(fit, total),
        dict_count: dict,
        dict_pct: pct(dict, total),
        fail_count: fail,
        fail_pct: pct(fail, total),
        fallback_5bit: fb5,
        fallback_6bit: fb6,
        avg_cost: avg,
        p50_cost: percentile_usize(&costs, 0.50),
        p90_cost: percentile_usize(&costs, 0.90),
        p95_cost: percentile_usize(&costs, 0.95),
        near_limit,
        real_fit,
        real_total,
        real_fit_pct: pct(real_fit, real_total),
        real_dict,
        real_dict_pct: pct(real_dict, real_total),
        real_fail,
        synthetic_fit: synth_fit,
        synthetic_total: synth_total,
        synthetic_fit_pct: pct(synth_fit, synth_total),
        deltas_vs_baseline: deltas,
        top_improved: improved.into_iter().map(|(_, t, s)| (t, s)).collect(),
        top_regressed: regressed.into_iter().map(|(_, t, s)| (t, s)).collect(),
    }
}

fn build_baseline(samples: &[Sample], pack: &TablePack) -> (ScenarioResult, Vec<(bool, usize)>) {
    // Treat baseline as a scenario named "baseline" but with the production
    // pack (no changes applied).
    let mut fit = 0usize;
    let mut fail = 0usize;
    let mut dict = 0usize;
    let mut fb5 = 0usize;
    let mut fb6 = 0usize;
    let mut costs: Vec<usize> = Vec::new();
    let mut near_limit = 0usize;
    let mut real_fit = 0usize;
    let mut real_total = 0usize;
    let mut real_dict = 0usize;
    let mut real_fail = 0usize;
    let mut synth_fit = 0usize;
    let mut synth_total = 0usize;
    let mut per_sample: Vec<(bool, usize)> = Vec::with_capacity(samples.len());

    for sample in samples {
        let res = simulate(&sample.text, pack, !is_synthetic_source(&sample.source));
        let is_real = !is_synthetic_source(&sample.source);
        per_sample.push((res.fits, res.cost));
        if is_real {
            real_total += 1;
        } else {
            synth_total += 1;
        }
        if res.fits {
            fit += 1;
            if is_real {
                real_fit += 1;
            } else {
                synth_fit += 1;
            }
            costs.push(res.cost);
            if res.cost >= 24 {
                near_limit += 1;
            }
            match res.mode {
                SimMode::Dict(_) => {
                    dict += 1;
                    if is_real {
                        real_dict += 1;
                    }
                }
                SimMode::Fallback5bit => fb5 += 1,
                SimMode::Fallback6bit => fb6 += 1,
            }
        } else {
            fail += 1;
            if is_real {
                real_fail += 1;
            }
        }
    }
    costs.sort_unstable();
    let total = samples.len();
    let avg = if !costs.is_empty() {
        costs.iter().sum::<usize>() as f64 / costs.len() as f64
    } else {
        0.0
    };

    let deltas: BTreeMap<String, f64> = BTreeMap::new();
    let scenario_result = ScenarioResult {
        name: "baseline".to_string(),
        description: format!(
            "Production HOXSHTV1.0 dict pack (fingerprint {}).",
            table_identity().fingerprint_sha256
        ),
        additions: vec![],
        replacements: vec![],
        fit_count: fit,
        fit_pct: pct(fit, total),
        dict_count: dict,
        dict_pct: pct(dict, total),
        fail_count: fail,
        fail_pct: pct(fail, total),
        fallback_5bit: fb5,
        fallback_6bit: fb6,
        avg_cost: avg,
        p50_cost: percentile_usize(&costs, 0.50),
        p90_cost: percentile_usize(&costs, 0.90),
        p95_cost: percentile_usize(&costs, 0.95),
        near_limit,
        real_fit,
        real_total,
        real_fit_pct: pct(real_fit, real_total),
        real_dict,
        real_dict_pct: pct(real_dict, real_total),
        real_fail,
        synthetic_fit: synth_fit,
        synthetic_total: synth_total,
        synthetic_fit_pct: pct(synth_fit, synth_total),
        deltas_vs_baseline: deltas,
        top_improved: vec![],
        top_regressed: vec![],
    };
    (scenario_result, per_sample)
}

struct CliArgs {
    scenarios: PathBuf,
    corpus_dirs: Vec<PathBuf>,
    out_dir: PathBuf,
    run_label: Option<String>,
    clean_out_dir: bool,
}

fn parse_args() -> Result<CliArgs, String> {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut scenarios: Option<PathBuf> = None;
    let mut corpus_dirs: Vec<PathBuf> = Vec::new();
    let mut out_dir = PathBuf::from("untracked/commander-v1.4/short-message-dry-run");
    let mut run_label: Option<String> = None;
    let mut clean_out_dir = false;
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        match a.as_str() {
            "--scenarios" => {
                let v = args
                    .get(i + 1)
                    .ok_or_else(|| "--scenarios requires a PATH".to_string())?;
                scenarios = Some(PathBuf::from(v));
                i += 2;
            }
            "--corpus-dir" => {
                let v = args
                    .get(i + 1)
                    .ok_or_else(|| "--corpus-dir requires a PATH".to_string())?;
                corpus_dirs.push(PathBuf::from(v));
                i += 2;
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
            other => return Err(format!("unknown flag: {}", other)),
        }
    }
    let scenarios = scenarios.ok_or_else(|| "--scenarios PATH is required".to_string())?;
    Ok(CliArgs {
        scenarios,
        corpus_dirs,
        out_dir,
        run_label,
        clean_out_dir,
    })
}

fn print_help() {
    println!("short_message_tuning_dry_run");
    println!();
    println!("USAGE:");
    println!("    short_message_tuning_dry_run --scenarios PATH [--corpus-dir PATH ...]");
    println!();
    println!("FLAGS:");
    println!("    --scenarios PATH      JSON file describing scenarios to run.");
    println!("    --corpus-dir PATH     Load every *.txt file in PATH. May be repeated.");
    println!("    --out-dir PATH        Override the report output directory.");
    println!("    --run-label LABEL     Write reports under <out-dir>/runs/<LABEL>/.");
    println!("    --clean-out-dir       Remove the run output directory before writing.");
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

fn main() {
    let cli = match parse_args() {
        Ok(c) => c,
        Err(err) => {
            eprintln!("error: {}", err);
            std::process::exit(2);
        }
    };

    let scenarios_text = match fs::read_to_string(&cli.scenarios) {
        Ok(t) => t,
        Err(err) => {
            eprintln!("error: failed to read scenarios file: {}", err);
            std::process::exit(2);
        }
    };
    let scenarios_file: ScenariosFile = match serde_json::from_str(&scenarios_text) {
        Ok(f) => f,
        Err(err) => {
            eprintln!("error: failed to parse scenarios JSON: {}", err);
            std::process::exit(2);
        }
    };

    let effective_out_dir = match &cli.run_label {
        Some(label) => cli.out_dir.join("runs").join(label),
        None => cli.out_dir.clone(),
    };

    if cli.clean_out_dir {
        if let Err(err) = remove_dir_if_exists(&effective_out_dir) {
            eprintln!("error: could not clean run output dir: {}", err);
            std::process::exit(2);
        }
    }

    let mut all: Vec<Sample> = Vec::new();
    for dir in &cli.corpus_dirs {
        match load_corpus_dir(dir) {
            Ok(samples) => all.extend(samples),
            Err(err) => eprintln!("warning: {}", err),
        }
    }
    let (all, removed) = dedup(all);
    if removed > 0 {
        eprintln!("info: removed {} duplicate samples", removed);
    }
    eprintln!("info: loaded {} unique samples", all.len());

    let prod_pack = TablePack::from_production();
    let (baseline, baseline_per_sample) = build_baseline(&all, &prod_pack);

    let mut results: Vec<ScenarioResult> = Vec::new();
    for scenario in &scenarios_file.scenarios {
        let pack = prod_pack.apply(scenario);
        let mut result = run_scenario(
            &scenario.name,
            &scenario.description,
            &pack,
            &all,
            &baseline,
            &baseline_per_sample,
        );
        result.additions = scenario.additions.clone();
        result.replacements = scenario.replacements.clone();
        results.push(result);
    }

    if let Err(err) = ensure_dir(&effective_out_dir) {
        eprintln!("error: could not create output dir: {}", err);
        std::process::exit(2);
    }
    let stamp = timestamp_stamp();
    let md_path = effective_out_dir.join(format!("short-message-dry-run-{}.md", stamp));
    let json_path = effective_out_dir.join(format!("short-message-dry-run-{}.json", stamp));

    let report = DryRunReport {
        generated_at_unix: now_unix(),
        generated_at_iso: now_iso8601(),
        run_label: cli.run_label.clone(),
        baseline: baseline.clone(),
        scenarios: results.clone(),
        table_fingerprint: table_identity().fingerprint_sha256,
        version_marker: HOXSHT_VERSION_MARKER.to_string(),
        corpus_total: all.len(),
    };

    if let Err(err) = write_markdown_report(&md_path, &report) {
        eprintln!("error: failed to write markdown report: {}", err);
        std::process::exit(2);
    }
    if let Err(err) = fs::write(
        &json_path,
        serde_json::to_string_pretty(&report).unwrap_or_else(|_| String::from("{}")),
    ) {
        eprintln!("error: failed to write json report: {}", err);
        std::process::exit(2);
    }

    println!(
        "short-message tuning dry run: {} samples, {} scenarios",
        all.len(),
        results.len()
    );
    println!(
        "baseline: fit={} ({:.2}%), dict={} ({:.2}%), fail={} ({:.2}%), real_fit={}/{} ({:.2}%)",
        baseline.fit_count,
        baseline.fit_pct,
        baseline.dict_count,
        baseline.dict_pct,
        baseline.fail_count,
        baseline.fail_pct,
        baseline.real_fit,
        baseline.real_total,
        baseline.real_fit_pct
    );
    for r in &results {
        println!(
            "{}: fit={} ({:+.2} pp), dict={} ({:+.2} pp), real_fit={:+.2} pp, real_dict={:+}",
            r.name,
            r.fit_count,
            r.deltas_vs_baseline.get("fit_pct").copied().unwrap_or(0.0),
            r.dict_count,
            r.deltas_vs_baseline.get("dict_pct").copied().unwrap_or(0.0),
            r.deltas_vs_baseline
                .get("real_fit_pct")
                .copied()
                .unwrap_or(0.0),
            r.real_dict as i64 - baseline.real_dict as i64
        );
    }
    println!();
    println!("Reports written:");
    println!("  - {}", md_path.display());
    println!("  - {}", json_path.display());
}

#[derive(Debug, Serialize)]
struct DryRunReport {
    generated_at_unix: u64,
    generated_at_iso: String,
    run_label: Option<String>,
    baseline: ScenarioResult,
    scenarios: Vec<ScenarioResult>,
    table_fingerprint: String,
    version_marker: String,
    corpus_total: usize,
}

fn write_markdown_report(path: &Path, report: &DryRunReport) -> std::io::Result<()> {
    let mut f = fs::File::create(path)?;
    writeln!(f, "# Short Message Dictionary Tuning Dry Run")?;
    writeln!(f)?;
    writeln!(
        f,
        "- Generated: {} (unix={})",
        report.generated_at_iso, report.generated_at_unix
    )?;
    if let Some(label) = &report.run_label {
        writeln!(f, "- Run label: `{}`", label)?;
    }
    writeln!(f, "- Table pack version: `{}`", report.version_marker)?;
    writeln!(
        f,
        "- Table pack fingerprint: `{}`",
        report.table_fingerprint
    )?;
    writeln!(f, "- Corpus total: {} unique samples", report.corpus_total)?;
    writeln!(f)?;
    writeln!(
        f,
        "> This is a DRY RUN. The production table pack has not been edited."
    )?;
    writeln!(
        f,
        "> Numbers are produced by a longest-prefix-match simulation. They are accurate enough to compare scenarios, not to predict exact hex output."
    )?;
    writeln!(f)?;
    writeln!(f, "## Baseline (production pack)")?;
    writeln!(f)?;
    write_scenario_table(&mut f, &report.baseline)?;
    writeln!(f)?;

    writeln!(f, "## Scenario Comparison")?;
    writeln!(f)?;
    writeln!(f, "| scenario | fit % | dict % | fail % | real fit % | real dict | avg cost | near limit | description |")?;
    writeln!(f, "|---|---|---|---|---|---|---|---|---|")?;
    writeln!(
        f,
        "| `baseline` | {:.2} | {:.2} | {:.2} | {:.2} | {} | {:.2} | {} | {} |",
        report.baseline.fit_pct,
        report.baseline.dict_pct,
        report.baseline.fail_pct,
        report.baseline.real_fit_pct,
        report.baseline.real_dict,
        report.baseline.avg_cost,
        report.baseline.near_limit,
        report.baseline.description.replace('|', "\\|"),
    )?;
    for s in &report.scenarios {
        writeln!(
            f,
            "| `{}` | {:.2} ({:+.2}) | {:.2} ({:+.2}) | {:.2} ({:+.2}) | {:.2} ({:+.2}) | {} ({:+}) | {:.2} ({:+.2}) | {} ({:+}) | {} |",
            s.name,
            s.fit_pct,
            s.deltas_vs_baseline.get("fit_pct").copied().unwrap_or(0.0),
            s.dict_pct,
            s.deltas_vs_baseline.get("dict_pct").copied().unwrap_or(0.0),
            s.fail_pct,
            s.deltas_vs_baseline.get("fail_pct").copied().unwrap_or(0.0),
            s.real_fit_pct,
            s.deltas_vs_baseline.get("real_fit_pct").copied().unwrap_or(0.0),
            s.real_dict,
            s.real_dict as i64 - report.baseline.real_dict as i64,
            s.avg_cost,
            s.deltas_vs_baseline.get("avg_cost").copied().unwrap_or(0.0),
            s.near_limit,
            s.near_limit as i64 - report.baseline.near_limit as i64,
            s.description.replace('|', "\\|"),
        )?;
    }
    writeln!(f)?;

    for s in &report.scenarios {
        writeln!(f, "## Scenario `{}`", s.name)?;
        writeln!(f)?;
        writeln!(f, "{}", s.description)?;
        writeln!(f)?;
        if !s.additions.is_empty() {
            writeln!(f, "Additions:")?;
            for a in &s.additions {
                writeln!(
                    f,
                    "  - DICT_{}: `{}`",
                    a.dict,
                    a.surface.replace('|', "\\|")
                )?;
            }
            writeln!(f)?;
        }
        if !s.replacements.is_empty() {
            writeln!(f, "Replacements:")?;
            for r in &s.replacements {
                writeln!(
                    f,
                    "  - DICT_{}: `{}` -> `{}`",
                    r.dict,
                    r.existing.replace('|', "\\|"),
                    r.replacement.replace('|', "\\|")
                )?;
            }
            writeln!(f)?;
        }
        write_scenario_table(&mut f, s)?;

        if !s.top_improved.is_empty() {
            writeln!(f, "### Top improved messages")?;
            for (txt, src) in &s.top_improved {
                writeln!(f, "  - `{}` (source: {})", txt.replace('|', "\\|"), src)?;
            }
            writeln!(f)?;
        }
        if !s.top_regressed.is_empty() {
            writeln!(f, "### Top regressed messages")?;
            for (txt, src) in &s.top_regressed {
                writeln!(f, "  - `{}` (source: {})", txt.replace('|', "\\|"), src)?;
            }
            writeln!(f)?;
        }
    }

    writeln!(f, "## Footnotes")?;
    writeln!(
        f,
        "- Simulation: greedy left-to-right longest-prefix-match across all 8 scenario dicts."
    )?;
    writeln!(
        f,
        "- Token cost: 1 byte. 5bit fallback: ceil(len*5/8) bytes. 6bit fallback: ceil(len*6/8) bytes."
    )?;
    writeln!(
        f,
        "- PAYLOAD_MAX = 27 bytes. `fits = cost <= 27`. `near_limit = cost >= 24`."
    )?;
    writeln!(
        f,
        "- Stem+suffix pairs, digit-run packing, and 2-byte literal escapes are NOT modeled. The simulation slightly under-counts dict coverage relative to the production encoder; the scenario-vs-baseline deltas are still directly comparable."
    )?;
    Ok(())
}

fn write_scenario_table(f: &mut fs::File, s: &ScenarioResult) -> std::io::Result<()> {
    writeln!(f, "| Metric | Value |")?;
    writeln!(f, "|---|---|")?;
    writeln!(f, "| Fit | {} ({:.2}%) |", s.fit_count, s.fit_pct)?;
    writeln!(f, "| Dict | {} ({:.2}%) |", s.dict_count, s.dict_pct)?;
    writeln!(f, "| Fail | {} ({:.2}%) |", s.fail_count, s.fail_pct)?;
    writeln!(f, "| Fallback 5bit | {} |", s.fallback_5bit)?;
    writeln!(f, "| Fallback 6bit | {} |", s.fallback_6bit)?;
    writeln!(
        f,
        "| Real (user-import) fit | {}/{} ({:.2}%) |",
        s.real_fit, s.real_total, s.real_fit_pct
    )?;
    writeln!(
        f,
        "| Real (user-import) dict | {} ({:.2}%) |",
        s.real_dict,
        pct(s.real_dict, s.real_total)
    )?;
    writeln!(
        f,
        "| Real (user-import) fail | {} ({:.2}%) |",
        s.real_fail,
        pct(s.real_fail, s.real_total)
    )?;
    writeln!(
        f,
        "| Synthetic fit | {}/{} ({:.2}%) |",
        s.synthetic_fit, s.synthetic_total, s.synthetic_fit_pct
    )?;
    writeln!(f, "| Avg cost | {:.2} |", s.avg_cost)?;
    writeln!(f, "| p50 cost | {} |", s.p50_cost)?;
    writeln!(f, "| p90 cost | {} |", s.p90_cost)?;
    writeln!(f, "| p95 cost | {} |", s.p95_cost)?;
    writeln!(f, "| Near limit (cost >= 24) | {} |", s.near_limit)?;
    Ok(())
}

#[allow(dead_code)]
fn _unused() {
    let _ = DICT_A;
    let _ = DICT_B;
    let _ = DICT_C;
    let _ = DICT_D;
    let _ = DICT_E;
    let _ = DICT_F;
    let _ = DICT_G;
    let _ = DICT_H;
    let _ = SUFFIXES;
    let _ = BTreeSet::<String>::new();
    let _ = HashMap::<String, String>::new();
}
