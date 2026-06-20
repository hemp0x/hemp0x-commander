// Custom short-message table-pack support.
//
// The Commander short-message codec is built on top of fixed-size 256-entry
// dictionaries. The expanded HS/HX header can address up to 16 dictionary slots
// while keeping the same 32-byte frame size. This lets operators import,
// export, and select custom table packs locally without touching chain or peer
// code.
//
// Built-in official HOXSHTV1.0 dictionaries stay the default. Custom packs are
// opt-in, validated strictly, and live under the Commander settings/data area
// so they follow custom data dir moves and settings migrations.
//
// The fingerprint format mirrors the algorithm already used by
// `short_message_tables::table_identity`. That way external audit tooling can
// verify a custom pack the same way it verifies the built-in pack.

use super::files::active_data_dir;
use super::short_message_tables::{
    ACRONYMS, ALPHABET_5BIT, ALPHABET_6BIT, DICT_A, DICT_B, DICT_C, DICT_D, DICT_E, DICT_F, DICT_G,
    DICT_H, DICT_I, DICT_J, DICT_K, DICT_L, DICT_M, DICT_N, DICT_O, DICT_P, HOXSHT_VERSION_MARKER,
    SUFFIXES,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

pub const MAX_DICTIONARIES: usize = 16;
pub const DICT_ENTRY_COUNT: usize = 256;
pub const DICT_LITERAL_ESCAPE: u8 = 255;
pub const TABLE_PACK_FILE_MAGIC: &str = "HEMP0X_TABLE_PACK";
pub const TABLE_PACK_FILE_VERSION: u32 = 1;
pub const CUSTOM_TABLE_PACK_VERSION_MARKER: &str = "CUSTOM_TABLE_PACK_V1";

const BUILT_IN_DICTIONARY_TITLES: [&str; MAX_DICTIONARIES] = [
    "DICT_A: primary general-purpose conversation.",
    "DICT_B: secondary general-purpose conversation.",
    "DICT_C: operations, coordination, workflow, and support.",
    "DICT_D: commerce, finance, orders, and settlement.",
    "DICT_E: general phrase pack.",
    "DICT_F: asset-owner, holder, and announcement phrase pack.",
    "DICT_G: traceability, logistics, and provenance.",
    "DICT_H: crypto, Hemp0x, on-chain, and technical language.",
    "DICT_I: reserved for future tuning.",
    "DICT_J: reserved for future tuning.",
    "DICT_K: reserved for future tuning.",
    "DICT_L: reserved for future tuning.",
    "DICT_M: reserved for future tuning.",
    "DICT_N: reserved for future tuning.",
    "DICT_O: reserved for future tuning.",
    "DICT_P: reserved for future tuning.",
];

/// Wire-format for an importable custom table pack file (JSON).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TablePackFile {
    pub magic: String,
    pub file_version: u32,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    /// Human-readable notes for dictionary slots. The codec ignores these; they
    /// exist so exported packs are easier to tune by hand.
    #[serde(default)]
    pub dictionary_titles: Vec<String>,
    /// Up to 16 dictionaries, in slot order A..P.
    pub dictionaries: Vec<Vec<String>>,
    /// Suffixes that the decoder treats as stem-attaching suffixes.
    #[serde(default)]
    pub suffixes: Vec<String>,
    /// Optional acronym list used by the decoder to restore casing.
    #[serde(default)]
    pub acronyms: Vec<String>,
}

/// The runtime representation of a single table pack. Built-in and custom
/// packs are both stored in this shape so the encoder/decoder does not have to
/// branch on origin.
#[derive(Debug, Clone)]
pub struct ValidatedTablePack {
    pub origin: TablePackOrigin,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    /// Optional display name for each dictionary slot, e.g. "A", "B", ...
    pub dictionary_names: Vec<String>,
    /// Normalized, lowercased entries of length DICT_ENTRY_COUNT. Slot 255 is
    /// always empty (reserved for the literal escape).
    pub dictionaries: Vec<Vec<String>>,
    /// Normalized suffixes.
    pub suffixes: Vec<String>,
    /// Normalized acronyms.
    pub acronyms: Vec<String>,
    /// SHA-256 fingerprint of the packed contents (same algorithm as
    /// `short_message_tables::table_identity`).
    pub fingerprint_sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TablePackOrigin {
    Builtin,
    Custom,
}

#[derive(Debug, Clone, Serialize)]
pub struct TablePackSummary {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub origin: TablePackOrigin,
    pub fingerprint_sha256: String,
    pub dictionary_count: usize,
    pub entry_counts: Vec<usize>,
    pub suffix_count: usize,
    pub acronym_count: usize,
    /// True for the pack currently feeding the encoder/decoder.
    pub active: bool,
    /// True for the built-in official pack, which is always present and cannot
    /// be deleted.
    pub builtin: bool,
    /// Local file path. None for the built-in pack.
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TablePackStatus {
    pub active: TablePackSummary,
    pub built_in: TablePackSummary,
    pub packs_dir: String,
    pub selection_path: String,
}

#[derive(Debug)]
pub struct TablePackError {
    pub message: String,
}

impl std::fmt::Display for TablePackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for TablePackError {}

impl From<String> for TablePackError {
    fn from(message: String) -> Self {
        Self { message }
    }
}

impl From<&str> for TablePackError {
    fn from(message: &str) -> Self {
        Self {
            message: message.to_string(),
        }
    }
}

fn normalize_phrase(phrase: &str) -> String {
    phrase.chars().flat_map(|c| c.to_lowercase()).collect()
}

fn contains_control_characters(text: &str) -> bool {
    text.chars().any(|ch| ch.is_control())
}

fn looks_like_word_missing_trailing_space(entry: &str) -> bool {
    // Plain multi-character alphabetic word that does not end in whitespace or
    // punctuation. Suggests the author forgot the conventional trailing space.
    if entry.len() < 4 {
        return false;
    }
    if entry.ends_with(' ') || entry.ends_with('\t') {
        return false;
    }
    let last = entry.chars().last().unwrap_or(' ');
    if !last.is_alphanumeric() {
        return false;
    }
    let letters = entry.chars().filter(|c| c.is_alphabetic()).count();
    let total = entry.chars().count();
    letters >= 4 && letters * 2 >= total
}

/// Validate a `TablePackFile` and turn it into a `ValidatedTablePack`.
///
/// This enforces the strict rules described in the slice spec:
/// - At most 16 dictionaries.
/// - Each dictionary has exactly 256 entries.
/// - Token 255 is empty (reserved for the literal escape).
/// - No duplicate non-empty entries per dictionary.
/// - No control characters in any entry.
/// - Suspicious entries (plain words missing trailing space) are returned as
///   warnings rather than hard errors.
pub fn validate_table_pack(
    file: &TablePackFile,
) -> Result<(ValidatedTablePack, Vec<String>), TablePackError> {
    let mut warnings = Vec::new();

    if file.magic.trim() != TABLE_PACK_FILE_MAGIC {
        return Err(format!(
            "Invalid magic: expected {:?}, got {:?}",
            TABLE_PACK_FILE_MAGIC, file.magic
        )
        .into());
    }

    if file.file_version != TABLE_PACK_FILE_VERSION {
        return Err(format!(
            "Unsupported file version: {} (expected {})",
            file.file_version, TABLE_PACK_FILE_VERSION
        )
        .into());
    }

    let name = file.name.trim();
    if name.is_empty() {
        return Err("Pack name is required".into());
    }
    if name.len() > 80 {
        return Err("Pack name is too long (max 80 characters)".into());
    }

    let version = file.version.trim();
    if version.is_empty() {
        return Err("Pack version label is required".into());
    }
    if version.len() > 40 {
        return Err("Pack version label is too long (max 40 characters)".into());
    }

    if file.dictionaries.is_empty() {
        return Err("At least one dictionary is required".into());
    }
    if file.dictionaries.len() > MAX_DICTIONARIES {
        return Err(format!(
            "Too many dictionaries: {} (max {})",
            file.dictionaries.len(),
            MAX_DICTIONARIES
        )
        .into());
    }

    let slot_names: Vec<String> = (0..file.dictionaries.len())
        .map(|idx| ((b'A' + idx as u8) as char).to_string())
        .collect();

    let mut normalized_dicts: Vec<Vec<String>> = Vec::with_capacity(file.dictionaries.len());

    for (dict_idx, dict) in file.dictionaries.iter().enumerate() {
        if dict.len() != DICT_ENTRY_COUNT {
            return Err(format!(
                "Dictionary {} must have exactly {} entries (got {})",
                dict_idx,
                DICT_ENTRY_COUNT,
                dict.len()
            )
            .into());
        }

        let slot_name = slot_names[dict_idx].clone();
        let mut normalized: Vec<String> = Vec::with_capacity(DICT_ENTRY_COUNT);
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

        for (entry_idx, raw) in dict.iter().enumerate() {
            if entry_idx == DICT_LITERAL_ESCAPE as usize {
                if !raw.is_empty() {
                    return Err(format!(
                        "Dictionary {} token {} must be empty (reserved for literal escape)",
                        slot_name, entry_idx
                    )
                    .into());
                }
                normalized.push(String::new());
                continue;
            }

            if raw.is_empty() {
                normalized.push(String::new());
                continue;
            }

            if contains_control_characters(raw) {
                return Err(format!(
                    "Dictionary {} entry {} contains control characters",
                    slot_name, entry_idx
                )
                .into());
            }

            if looks_like_word_missing_trailing_space(raw) {
                warnings.push(format!(
                    "Dictionary {} entry {} ({:?}) looks like a normal word but has no trailing space",
                    slot_name, entry_idx, raw
                ));
            }

            let normalized_entry = normalize_phrase(raw);
            if !seen.insert(normalized_entry.clone()) {
                return Err(format!(
                    "Dictionary {} has duplicate entry {:?} (entries must be unique except for the reserved empty slot)",
                    slot_name, raw
                )
                .into());
            }

            normalized.push(normalized_entry);
        }

        // Token 255 must be empty after validation.
        debug_assert!(normalized[DICT_LITERAL_ESCAPE as usize].is_empty());

        normalized_dicts.push(normalized);
    }

    let mut normalized_suffixes: Vec<String> = Vec::with_capacity(file.suffixes.len());
    let mut seen_suffixes: std::collections::HashSet<String> = std::collections::HashSet::new();
    for raw in &file.suffixes {
        if raw.is_empty() {
            continue;
        }
        if contains_control_characters(raw) {
            return Err(format!("Suffix {:?} contains control characters", raw).into());
        }
        let normalized_suffix = normalize_phrase(raw);
        if !seen_suffixes.insert(normalized_suffix.clone()) {
            return Err(format!("Duplicate suffix {:?}", raw).into());
        }
        normalized_suffixes.push(normalized_suffix);
    }

    let mut normalized_acronyms: Vec<String> = Vec::with_capacity(file.acronyms.len());
    let mut seen_acronyms: std::collections::HashSet<String> = std::collections::HashSet::new();
    for raw in &file.acronyms {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        if contains_control_characters(trimmed) {
            return Err(format!("Acronym {:?} contains control characters", trimmed).into());
        }
        if !seen_acronyms.insert(trimmed.to_string()) {
            return Err(format!("Duplicate acronym {:?}", trimmed).into());
        }
        normalized_acronyms.push(normalize_phrase(trimmed));
    }

    let fingerprint = compute_table_pack_fingerprint(
        CUSTOM_TABLE_PACK_VERSION_MARKER,
        &slot_names,
        &normalized_dicts,
        &normalized_suffixes,
    );

    let pack = ValidatedTablePack {
        origin: TablePackOrigin::Custom,
        name: name.to_string(),
        version: version.to_string(),
        description: file
            .description
            .as_ref()
            .map(|d| d.trim().to_string())
            .filter(|d| !d.is_empty()),
        dictionary_names: slot_names,
        dictionaries: normalized_dicts,
        suffixes: normalized_suffixes,
        acronyms: normalized_acronyms,
        fingerprint_sha256: fingerprint,
    };

    Ok((pack, warnings))
}

/// Build the validated built-in pack directly from the compiled-in
/// `short_message_tables` constants. The fingerprint matches
/// `short_message_tables::table_identity()` exactly because the same hashing
/// rules are applied.
pub fn built_in_table_pack() -> ValidatedTablePack {
    let raw_dicts: [&[&str; DICT_ENTRY_COUNT]; MAX_DICTIONARIES] = [
        &DICT_A, &DICT_B, &DICT_C, &DICT_D, &DICT_E, &DICT_F, &DICT_G, &DICT_H, &DICT_I, &DICT_J,
        &DICT_K, &DICT_L, &DICT_M, &DICT_N, &DICT_O, &DICT_P,
    ];

    let mut dictionaries: Vec<Vec<String>> = Vec::with_capacity(MAX_DICTIONARIES);
    for dict in &raw_dicts {
        let entries: Vec<String> = dict.iter().map(|s| normalize_phrase(s)).collect();
        dictionaries.push(entries);
    }

    let dictionary_names: Vec<String> = (0..MAX_DICTIONARIES)
        .map(|idx| ((b'A' + idx as u8) as char).to_string())
        .collect();

    let suffixes: Vec<String> = SUFFIXES.iter().map(|s| normalize_phrase(s)).collect();
    let acronyms: Vec<String> = ACRONYMS.iter().map(|s| normalize_phrase(s)).collect();

    let fingerprint = compute_table_pack_fingerprint(
        HOXSHT_VERSION_MARKER,
        &dictionary_names,
        &dictionaries,
        &suffixes,
    );

    ValidatedTablePack {
        origin: TablePackOrigin::Builtin,
        name: HOXSHT_VERSION_MARKER.to_string(),
        version: HOXSHT_VERSION_MARKER.to_string(),
        description: Some(
            "Official HOXSHTV1.0 short-message table pack compiled into Commander.".to_string(),
        ),
        dictionary_names,
        dictionaries,
        suffixes,
        acronyms,
        fingerprint_sha256: fingerprint,
    }
}

fn compute_table_pack_fingerprint(
    version_marker: &str,
    dictionary_names: &[String],
    dictionaries: &[Vec<String>],
    suffixes: &[String],
) -> String {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(version_marker.as_bytes());
    hasher.update([0u8]);

    for (idx, dict) in dictionaries.iter().enumerate() {
        let name = dictionary_names.get(idx).map(|s| s.as_str()).unwrap_or("");
        hasher.update(name.as_bytes());
        hasher.update([0u8]);
        let count = dict.len() as u32;
        hasher.update(count.to_le_bytes());
        for phrase in dict {
            let len = phrase.len() as u32;
            hasher.update(len.to_le_bytes());
            hasher.update(phrase.as_bytes());
            hasher.update([0x1fu8]);
        }
    }

    for suffix in suffixes {
        let len = suffix.len() as u32;
        hasher.update(len.to_le_bytes());
        hasher.update(suffix.as_bytes());
        hasher.update([0x1fu8]);
    }

    let digest = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(&mut hex, "{:02x}", byte);
    }
    hex
}

pub fn compute_built_in_fingerprint() -> String {
    let pack = built_in_table_pack();
    pack.fingerprint_sha256
}

fn packs_dir_impl() -> Result<PathBuf, String> {
    let dir = active_data_dir()?.join("commander").join("table-packs");
    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    Ok(dir)
}

fn selection_path_impl() -> Result<PathBuf, String> {
    Ok(active_data_dir()?
        .join("commander")
        .join("active_table_pack.json"))
}

fn sanitize_id(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        out.push_str("pack");
    }
    out
}

fn build_summary(
    pack: &ValidatedTablePack,
    active: bool,
    path: Option<String>,
) -> TablePackSummary {
    let entry_counts: Vec<usize> = pack.dictionaries.iter().map(|d| d.len()).collect();
    TablePackSummary {
        id: sanitize_id(&pack.name),
        name: pack.name.clone(),
        version: pack.version.clone(),
        description: pack.description.clone(),
        origin: pack.origin,
        fingerprint_sha256: pack.fingerprint_sha256.clone(),
        dictionary_count: pack.dictionaries.len(),
        entry_counts,
        suffix_count: pack.suffixes.len(),
        acronym_count: pack.acronyms.len(),
        active,
        builtin: matches!(pack.origin, TablePackOrigin::Builtin),
        path,
    }
}

pub(crate) struct PackState {
    pub(crate) active: ValidatedTablePack,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TablePackSelection {
    #[default]
    Builtin,
    Custom {
        name: String,
        version: String,
        fingerprint_sha256: String,
    },
}

static PACK_STATE: OnceLock<Mutex<Arc<PackState>>> = OnceLock::new();

fn pack_state() -> &'static Mutex<Arc<PackState>> {
    PACK_STATE.get_or_init(|| Mutex::new(Arc::new(initial_state())))
}

fn initial_state() -> PackState {
    let builtin = built_in_table_pack();
    let selection = read_selection_from_disk().unwrap_or_default();
    if let TablePackSelection::Custom {
        name,
        version,
        fingerprint_sha256,
    } = &selection
    {
        if let Ok(Some(pack)) = find_custom_pack(name, version, fingerprint_sha256) {
            return PackState { active: pack };
        }
    }
    PackState {
        active: builtin,
    }
}

fn read_selection_from_disk() -> Option<TablePackSelection> {
    let path = selection_path_impl().ok()?;
    if !path.exists() {
        return None;
    }
    let content = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

fn write_selection_to_disk(selection: &TablePackSelection) -> Result<(), String> {
    let path = selection_path_impl()?;
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }
    let content = serde_json::to_string_pretty(selection).map_err(|e| e.to_string())?;
    fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(())
}

fn set_active_state(pack: ValidatedTablePack, _selection: TablePackSelection) {
    let new_state = Arc::new(PackState {
        active: pack,
    });
    let mut guard = pack_state().lock().expect("pack state poisoned");
    *guard = new_state;
}

pub(crate) fn snapshot_active_pack() -> Arc<PackState> {
    let guard = pack_state().lock().expect("pack state poisoned");
    Arc::clone(&guard)
}

pub fn active_pack() -> ValidatedTablePack {
    snapshot_active_pack().active.clone()
}

pub fn active_table_identity() -> ActiveTableIdentity {
    let state = snapshot_active_pack();
    let pack = &state.active;
    let entry_counts: Vec<(String, usize)> = pack
        .dictionaries
        .iter()
        .enumerate()
        .map(|(idx, dict)| {
            (
                pack.dictionary_names
                    .get(idx)
                    .cloned()
                    .unwrap_or_else(|| ((b'A' + idx as u8) as char).to_string()),
                dict.len(),
            )
        })
        .collect();
    ActiveTableIdentity {
        name: pack.name.clone(),
        version: pack.version.clone(),
        origin: pack.origin,
        fingerprint_sha256: pack.fingerprint_sha256.clone(),
        dictionary_count: pack.dictionaries.len(),
        entry_counts,
        suffix_count: pack.suffixes.len(),
        acronym_count: pack.acronyms.len(),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ActiveTableIdentity {
    pub name: String,
    pub version: String,
    pub origin: TablePackOrigin,
    pub fingerprint_sha256: String,
    pub dictionary_count: usize,
    pub entry_counts: Vec<(String, usize)>,
    pub suffix_count: usize,
    pub acronym_count: usize,
}

fn list_pack_files() -> Result<Vec<(PathBuf, ValidatedTablePack)>, String> {
    let dir = packs_dir_impl()?;
    let mut out = Vec::new();
    for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        match load_pack_from_path(&path) {
            Ok(pack) => out.push((path, pack)),
            Err(err) => {
                eprintln!("Skipping invalid table pack {}: {}", path.display(), err);
            }
        }
    }
    out.sort_by(|a, b| a.1.name.cmp(&b.1.name).then(a.1.version.cmp(&b.1.version)));
    Ok(out)
}

fn load_pack_from_path(path: &PathBuf) -> Result<ValidatedTablePack, String> {
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let file: TablePackFile = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    let (pack, _warnings) =
        validate_table_pack(&file).map_err(|e| format!("validation failed: {}", e.message))?;
    Ok(pack)
}

fn find_custom_pack(
    name: &str,
    version: &str,
    fingerprint_sha256: &str,
) -> Result<Option<ValidatedTablePack>, String> {
    let dir = packs_dir_impl()?;
    for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if let Ok(pack) = load_pack_from_path(&path) {
            if pack.name == name
                && pack.version == version
                && pack.fingerprint_sha256 == fingerprint_sha256
            {
                return Ok(Some(pack));
            }
        }
    }
    Ok(None)
}

fn pack_to_file(pack: &ValidatedTablePack) -> TablePackFile {
    TablePackFile {
        magic: TABLE_PACK_FILE_MAGIC.to_string(),
        file_version: TABLE_PACK_FILE_VERSION,
        name: pack.name.clone(),
        version: pack.version.clone(),
        description: pack.description.clone(),
        dictionary_titles: pack
            .dictionary_names
            .iter()
            .enumerate()
            .map(|(idx, name)| {
                BUILT_IN_DICTIONARY_TITLES
                    .get(idx)
                    .map(|title| title.to_string())
                    .unwrap_or_else(|| format!("DICT_{}: custom dictionary.", name))
            })
            .collect(),
        dictionaries: pack
            .dictionaries
            .iter()
            .map(|dict| {
                dict.iter()
                    .map(|phrase| {
                        if phrase.is_empty() {
                            String::new()
                        } else {
                            phrase.clone()
                        }
                    })
                    .collect()
            })
            .collect(),
        suffixes: pack.suffixes.clone(),
        acronyms: pack.acronyms.clone(),
    }
}

fn json_string(value: &str) -> Result<String, String> {
    serde_json::to_string(value).map_err(|e| e.to_string())
}

fn write_string_array(out: &mut String, values: &[String]) -> Result<(), String> {
    out.push('[');
    for (idx, value) in values.iter().enumerate() {
        if idx > 0 {
            out.push_str(", ");
        }
        out.push_str(&json_string(value)?);
    }
    out.push(']');
    Ok(())
}

fn table_pack_to_editable_json(file: &TablePackFile) -> Result<String, String> {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str("  \"magic\": ");
    out.push_str(&json_string(&file.magic)?);
    out.push_str(",\n  \"file_version\": ");
    out.push_str(&file.file_version.to_string());
    out.push_str(",\n  \"name\": ");
    out.push_str(&json_string(&file.name)?);
    out.push_str(",\n  \"version\": ");
    out.push_str(&json_string(&file.version)?);
    if let Some(description) = &file.description {
        out.push_str(",\n  \"description\": ");
        out.push_str(&json_string(description)?);
    }
    if !file.dictionary_titles.is_empty() {
        out.push_str(",\n  \"dictionary_titles\": ");
        write_string_array(&mut out, &file.dictionary_titles)?;
    }
    out.push_str(",\n  \"dictionaries\": [\n");
    for (idx, dict) in file.dictionaries.iter().enumerate() {
        out.push_str("    ");
        write_string_array(&mut out, dict)?;
        if idx + 1 != file.dictionaries.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str("  ],\n  \"suffixes\": ");
    write_string_array(&mut out, &file.suffixes)?;
    out.push_str(",\n  \"acronyms\": ");
    write_string_array(&mut out, &file.acronyms)?;
    out.push_str("\n}\n");
    Ok(out)
}

pub fn export_built_in_table_pack(target_path: &str) -> Result<PathBuf, String> {
    let pack = built_in_table_pack();
    let mut path = PathBuf::from(target_path);
    if path.is_dir() || target_path.ends_with('/') {
        path.push(format!(
            "{}-{}.json",
            sanitize_id(&pack.name),
            sanitize_id(&pack.version)
        ));
    }
    if path.extension().and_then(|e| e.to_str()).is_none() {
        path.set_extension("json");
    }
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }
    let file = pack_to_file(&pack);
    let content = table_pack_to_editable_json(&file)?;
    fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(path)
}

pub fn import_table_pack(source_path: &str) -> Result<TablePackSummary, String> {
    let src = PathBuf::from(source_path);
    let pack = load_pack_from_path(&src)?;
    let dir = packs_dir_impl()?;
    let base_id = sanitize_id(&pack.name);
    let mut target = dir.join(format!("{}-{}.json", base_id, sanitize_id(&pack.version)));
    let mut counter = 2u32;
    while target.exists() {
        target = dir.join(format!(
            "{}-{}-{}.json",
            base_id,
            sanitize_id(&pack.version),
            counter
        ));
        counter += 1;
    }
    let file = pack_to_file(&pack);
    let content = serde_json::to_string_pretty(&file).map_err(|e| e.to_string())?;
    fs::write(&target, content).map_err(|e| e.to_string())?;
    Ok(build_summary(
        &pack,
        false,
        Some(target.to_string_lossy().to_string()),
    ))
}

pub fn delete_custom_table_pack(
    name: &str,
    version: &str,
    fingerprint_sha256: &str,
) -> Result<TablePackSummary, String> {
    let dir = packs_dir_impl()?;
    let mut deleted = false;

    for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Ok(pack) = load_pack_from_path(&path) else {
            continue;
        };
        if pack.name == name
            && pack.version == version
            && pack.fingerprint_sha256 == fingerprint_sha256
        {
            fs::remove_file(&path).map_err(|e| e.to_string())?;
            deleted = true;
            break;
        }
    }

    if !deleted {
        return Err(format!(
            "Custom table pack not found: {} {} ({})",
            name, version, fingerprint_sha256
        ));
    }

    let active = snapshot_active_pack().active.clone();
    if active.name == name
        && active.version == version
        && active.fingerprint_sha256 == fingerprint_sha256
    {
        return reset_to_built_in_table_pack();
    }

    Ok(active_table_pack_summary())
}

pub fn select_active_table_pack(selection: TablePackSelection) -> Result<TablePackSummary, String> {
    let (new_active, new_selection) = match &selection {
        TablePackSelection::Builtin => (built_in_table_pack(), TablePackSelection::Builtin),
        TablePackSelection::Custom {
            name,
            version,
            fingerprint_sha256,
        } => {
            let pack = find_custom_pack(name, version, fingerprint_sha256)?.ok_or_else(|| {
                format!(
                    "Custom table pack not found: {} {} ({})",
                    name, version, fingerprint_sha256
                )
            })?;
            (pack, selection.clone())
        }
    };

    write_selection_to_disk(&new_selection)?;
    set_active_state(new_active.clone(), new_selection);

    Ok(build_summary(&new_active, true, None))
}

pub fn reset_to_built_in_table_pack() -> Result<TablePackSummary, String> {
    let builtin = built_in_table_pack();
    write_selection_to_disk(&TablePackSelection::Builtin)?;
    set_active_state(builtin.clone(), TablePackSelection::Builtin);
    Ok(build_summary(&builtin, true, None))
}

/// Test-only helper: install a validated pack as the active pack without
/// touching disk. Used by integration tests to verify that the encoder and
/// decoder follow a custom pack and that `reset_to_built_in_table_pack`
/// returns the system to the compiled defaults.
#[doc(hidden)]
pub fn set_active_pack_for_tests(pack: ValidatedTablePack) {
    set_active_state(pack, TablePackSelection::Builtin);
}

/// Single global mutex used by short-message tests to serialize access to
/// the active pack, runtime cache, and suggester cache. The runtime stores
/// the active pack in process-global state, so test isolation requires
/// running short-message encode/decode/suggest tests one at a time.
#[doc(hidden)]
pub fn test_serialize_lock() -> &'static std::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
}

pub fn list_table_packs() -> Result<Vec<TablePackSummary>, String> {
    let state = snapshot_active_pack();
    let active_fingerprint = state.active.fingerprint_sha256.clone();
    let active_name = state.active.name.clone();
    let active_version = state.active.version.clone();

    let builtin = built_in_table_pack();
    let mut summaries = Vec::new();

    let builtin_active = active_fingerprint == builtin.fingerprint_sha256
        && active_name == builtin.name
        && active_version == builtin.version;
    summaries.push(build_summary(&builtin, builtin_active, None));

    for (path, pack) in list_pack_files()? {
        let active = pack.fingerprint_sha256 == active_fingerprint
            && pack.name == active_name
            && pack.version == active_version;
        summaries.push(build_summary(
            &pack,
            active,
            Some(path.to_string_lossy().to_string()),
        ));
    }

    Ok(summaries)
}

pub fn get_active_table_pack_status() -> Result<TablePackStatus, String> {
    let summaries = list_table_packs()?;
    let active = summaries
        .iter()
        .find(|s| s.active)
        .cloned()
        .ok_or_else(|| "No active table pack found".to_string())?;
    let builtin = summaries
        .iter()
        .find(|s| s.builtin)
        .cloned()
        .ok_or_else(|| "Built-in table pack missing from status".to_string())?;
    Ok(TablePackStatus {
        active,
        built_in: builtin,
        packs_dir: packs_dir_impl()?.to_string_lossy().to_string(),
        selection_path: selection_path_impl()?.to_string_lossy().to_string(),
    })
}

pub fn active_table_pack_summary() -> TablePackSummary {
    let state = snapshot_active_pack();
    build_summary(&state.active, true, None)
}

pub fn built_in_summary() -> TablePackSummary {
    let builtin = built_in_table_pack();
    build_summary(&builtin, false, None)
}

// --- Tauri commands ---

#[tauri::command]
pub fn short_message_list_table_packs() -> Result<Vec<TablePackSummary>, String> {
    list_table_packs()
}

#[tauri::command]
pub fn short_message_get_active_table_pack() -> Result<TablePackStatus, String> {
    get_active_table_pack_status()
}

#[tauri::command]
pub fn short_message_import_table_pack(source_path: String) -> Result<TablePackSummary, String> {
    import_table_pack(&source_path)
}

#[tauri::command]
pub fn short_message_export_built_in_table_pack(target_path: String) -> Result<String, String> {
    let path = export_built_in_table_pack(&target_path)?;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn short_message_select_table_pack(
    name: String,
    version: String,
    fingerprint_sha256: String,
) -> Result<TablePackSummary, String> {
    select_active_table_pack(TablePackSelection::Custom {
        name,
        version,
        fingerprint_sha256,
    })
}

#[tauri::command]
pub fn short_message_delete_table_pack(
    name: String,
    version: String,
    fingerprint_sha256: String,
) -> Result<TablePackSummary, String> {
    delete_custom_table_pack(&name, &version, &fingerprint_sha256)
}

#[tauri::command]
pub fn short_message_reset_table_pack() -> Result<TablePackSummary, String> {
    reset_to_built_in_table_pack()
}

/// Re-export the constants the encoder needs to fall back to the built-in
/// fallback alphabets. Custom packs cannot change these because the 5-bit/6-bit
/// mode and suffix behavior are part of the wire format, not the dictionary
/// pack.
pub fn fallback_alphabets() -> (&'static [u8; 32], &'static [u8; 64]) {
    (&ALPHABET_5BIT, &ALPHABET_6BIT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::short_message_tables::table_identity;

    fn minimal_pack_dicts() -> Vec<String> {
        let mut dict = vec![String::new(); DICT_ENTRY_COUNT];
        dict[0] = " ".to_string();
        dict[1] = "hello ".to_string();
        dict[2] = "world ".to_string();
        dict
    }

    fn minimal_pack() -> TablePackFile {
        TablePackFile {
            magic: TABLE_PACK_FILE_MAGIC.to_string(),
            file_version: TABLE_PACK_FILE_VERSION,
            name: "Test Pack".to_string(),
            version: "1.0".to_string(),
            description: Some("Hello world test pack".to_string()),
            dictionary_titles: vec!["DICT_A: test dictionary.".to_string()],
            dictionaries: vec![minimal_pack_dicts(), minimal_pack_dicts()],
            suffixes: vec!["ed ".to_string()],
            acronyms: vec!["lol".to_string()],
        }
    }

    #[test]
    fn rejects_non_empty_token_255() {
        let mut file = minimal_pack();
        file.dictionaries[0][DICT_LITERAL_ESCAPE as usize] = " ".to_string();
        let err = validate_table_pack(&file).unwrap_err();
        assert!(err.message.contains("token 255"));
    }

    #[test]
    fn requires_token_255_empty() {
        let file = minimal_pack();
        let (pack, _) = validate_table_pack(&file).unwrap();
        assert!(pack.dictionaries[0][DICT_LITERAL_ESCAPE as usize].is_empty());
    }

    #[test]
    fn rejects_duplicate_entries() {
        let mut file = minimal_pack();
        file.dictionaries[0][10] = "hello ".to_string();
        let err = validate_table_pack(&file).unwrap_err();
        assert!(err.message.contains("duplicate"));
    }

    #[test]
    fn rejects_control_characters() {
        let mut file = minimal_pack();
        file.dictionaries[0][10] = "bad\u{0001}word".to_string();
        let err = validate_table_pack(&file).unwrap_err();
        assert!(err.message.contains("control"));
    }

    #[test]
    fn rejects_wrong_dict_count() {
        let mut file = minimal_pack();
        file.dictionaries[0] = vec![String::new(); 100];
        let err = validate_table_pack(&file).unwrap_err();
        assert!(err.message.contains("256"));
    }

    #[test]
    fn rejects_too_many_dictionaries() {
        let mut file = minimal_pack();
        for _ in 0..15 {
            file.dictionaries.push(minimal_pack_dicts());
        }
        let err = validate_table_pack(&file).unwrap_err();
        assert!(err.message.contains("Too many"));
    }

    #[test]
    fn warns_on_word_missing_trailing_space() {
        let mut file = minimal_pack();
        file.dictionaries[0][10] = "payment".to_string();
        let (_pack, warnings) = validate_table_pack(&file).unwrap();
        assert!(warnings.iter().any(|w| w.contains("trailing space")));
    }

    #[test]
    fn accepts_clean_pack() {
        let file = minimal_pack();
        let (pack, warnings) = validate_table_pack(&file).unwrap();
        assert_eq!(pack.name, "Test Pack");
        assert_eq!(pack.dictionaries.len(), 2);
        assert!(warnings.is_empty());
    }

    #[test]
    fn preserves_spaces_in_imported_entries() {
        let file = minimal_pack();
        let (pack, _warnings) = validate_table_pack(&file).unwrap();
        assert_eq!(pack.dictionaries[0][0], " ");
        assert_eq!(pack.dictionaries[0][1], "hello ");
        assert_eq!(pack.suffixes[0], "ed ");
    }

    #[test]
    fn built_in_pack_fingerprint_matches_legacy_identity() {
        let pack = built_in_table_pack();
        let legacy = table_identity();
        assert_eq!(pack.fingerprint_sha256, legacy.fingerprint_sha256);
        assert_eq!(pack.dictionaries.len(), 16);
    }

    #[test]
    fn export_built_in_round_trip() {
        let dir = std::env::temp_dir().join(format!(
            "hemp0x-table-pack-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("mkdir");
        let target = dir.join("built-in.json");
        let written = export_built_in_table_pack(target.to_str().unwrap()).expect("export");
        assert!(written.exists());
        let content = std::fs::read_to_string(&written).expect("read export");
        assert!(content.contains("\"dictionary_titles\": ["));
        assert!(content.contains("DICT_A: primary general-purpose conversation."));
        assert!(content.contains("\"dictionaries\": [\n    ["));
        assert!(!content.contains("\n      \"about \""));
        let reloaded = load_pack_from_path(&written).expect("reload");
        // The exported file becomes a "custom" pack, so its fingerprint is
        // anchored to the custom version marker. Verify the dictionary
        // contents round-tripped cleanly and the fingerprint is stable.
        assert_eq!(reloaded.dictionaries.len(), 16);
        assert_eq!(reloaded.dictionaries[0].len(), 256);
        assert!(reloaded.dictionaries[0][DICT_LITERAL_ESCAPE as usize].is_empty());
        let first = reloaded_for_fingerprint(&written);
        let second = reloaded_for_fingerprint(&written);
        assert_eq!(first, second);
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn reloaded_for_fingerprint(path: &std::path::PathBuf) -> String {
        let pack = load_pack_from_path(path).expect("reload");
        compute_table_pack_fingerprint(
            CUSTOM_TABLE_PACK_VERSION_MARKER,
            &pack.dictionary_names,
            &pack.dictionaries,
            &pack.suffixes,
        )
    }

    #[test]
    fn invalid_pack_is_rejected() {
        let mut file = minimal_pack();
        file.magic = "WRONG".to_string();
        let err = validate_table_pack(&file).unwrap_err();
        assert!(err.message.contains("magic"));
    }

    #[test]
    fn duplicate_entries_also_caught_in_other_slots() {
        let mut file = minimal_pack();
        file.dictionaries[1][5] = "hello ".to_string();
        let err = validate_table_pack(&file).unwrap_err();
        assert!(err.message.contains("duplicate"));
    }

    #[test]
    fn editable_json_preserves_one_line_dictionaries() {
        let file = pack_to_file(&built_in_table_pack());
        let content = table_pack_to_editable_json(&file).expect("format");
        assert!(content.contains("\"dictionary_titles\": ["));
        assert!(content.contains("DICT_H: crypto, Hemp0x, on-chain, and technical language."));
        assert!(content.contains("\"dictionaries\": [\n    ["));
        assert!(content.contains("\"suffixes\": ["));
        assert!(content.contains("\"acronyms\": ["));
        assert!(!content.contains("\n      \" "));
    }
}
