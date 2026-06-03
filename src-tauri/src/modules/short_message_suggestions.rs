use super::short_message;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

const MAX: usize = 6;
const DICT_COUNT: usize = 5;
const RECENT_CONTEXT_TOKENS: usize = 3;
const DOMAIN_HINT_MIN_SCORE: u16 = 2;

struct Meta {
    display: String,
    norm: String,
    first: String,
    dict: u8,
    word_count: u8,
}

struct Node {
    children: HashMap<u8, Box<Node>>,
    subtree_ids: Vec<u16>,
}

struct Suggester {
    root: Node,
    phrases: Vec<Meta>,
    token_domains: HashMap<String, [u16; DICT_COUNT]>,
}

static SUGGESTER: OnceLock<Suggester> = OnceLock::new();

fn suggester() -> &'static Suggester {
    SUGGESTER.get_or_init(build_suggester)
}

fn new_node(capacity: usize) -> Node {
    Node {
        children: HashMap::with_capacity(capacity),
        subtree_ids: Vec::new(),
    }
}

fn build_suggester() -> Suggester {
    let runtimes = short_message::get_dictionary_runtimes();
    let mut root = new_node(32);
    let mut phrases = Vec::with_capacity(1280);
    let mut token_domains = HashMap::with_capacity(1024);
    let mut phrase_id: u16 = 0;

    for (dict_idx, runtime) in runtimes.iter().enumerate() {
        for norm in &runtime.normalized {
            if norm.trim().is_empty() {
                continue;
            }

            let bytes = norm.as_bytes();
            let first = norm.split_whitespace().next().unwrap_or("").to_string();
            if first.is_empty() {
                continue;
            }

            root.subtree_ids.push(phrase_id);
            let mut node = &mut root;
            for &byte in bytes {
                node = node
                    .children
                    .entry(byte)
                    .or_insert_with(|| Box::new(new_node(8)));
                node.subtree_ids.push(phrase_id);
            }

            let mut seen = HashSet::new();
            for token in norm.split_whitespace() {
                if seen.insert(token) {
                    token_domains
                        .entry(token.to_string())
                        .or_insert([0; DICT_COUNT])[dict_idx] += 1;
                }
            }

            phrases.push(Meta {
                display: short_message::restore_case_pub(norm),
                norm: norm.clone(),
                first,
                dict: dict_idx as u8,
                word_count: bytes.iter().filter(|&&byte| byte == b' ').count() as u8 + 1,
            });
            phrase_id += 1;
        }
    }

    Suggester {
        root,
        phrases,
        token_domains,
    }
}

fn lookup_phrase_ids<'a>(root: &'a Node, prefix: &[u8]) -> &'a [u16] {
    let mut node = root;
    for &byte in prefix {
        match node.children.get(&byte) {
            Some(child) => node = child,
            None => return &[],
        }
    }
    &node.subtree_ids
}

fn is_punctuation_only(text: &str) -> bool {
    text.chars()
        .all(|ch| ch.is_ascii_punctuation() || ch.is_whitespace())
}

fn recent_context_tokens(context: &[String]) -> impl Iterator<Item = &str> {
    context
        .iter()
        .rev()
        .take(RECENT_CONTEXT_TOKENS)
        .map(String::as_str)
}

fn phrase_has_related_word(phrase: &str, token: &str) -> bool {
    phrase
        .split_whitespace()
        .any(|word| word == token || word.starts_with(token) || token.starts_with(word))
}

fn leading_context_overlap(meta: &Meta, context: &[String]) -> usize {
    let phrase_words: Vec<&str> = meta.norm.split_whitespace().collect();
    let max_overlap = context.len().min(phrase_words.len());
    for count in (1..=max_overlap).rev() {
        if context[context.len() - count..]
            .iter()
            .map(String::as_str)
            .eq(phrase_words[..count].iter().copied())
        {
            return count;
        }
    }
    0
}

fn domain_hint(suggester: &Suggester, context: &[String]) -> Option<u8> {
    let mut scores = [0u16; DICT_COUNT];
    for token in recent_context_tokens(context) {
        if let Some(weights) = suggester.token_domains.get(token) {
            for (idx, weight) in weights.iter().enumerate() {
                scores[idx] += *weight;
            }
        }
    }

    let (best_idx, best_score) = scores
        .iter()
        .enumerate()
        .max_by_key(|(_, score)| *score)?;
    (best_score >= &DOMAIN_HINT_MIN_SCORE).then_some(best_idx as u8)
}

fn phrase_bonus(word_count: u8) -> i32 {
    6 * i32::from(word_count.saturating_sub(1).min(3))
}

fn overlap_bonus(meta: &Meta, context: &[String]) -> i32 {
    recent_context_tokens(context)
        .filter(|token| phrase_has_related_word(&meta.norm, token))
        .count() as i32
        * 10
}

fn sequence_bonus(meta: &Meta, context: &[String]) -> i32 {
    let Some(last_token) = context.last().map(String::as_str) else {
        return 0;
    };

    if meta.first == last_token {
        24
    } else if meta.word_count > 1 && meta.norm.split_whitespace().skip(1).any(|word| word == last_token) {
        12
    } else {
        0
    }
}

fn continuation_bonus(meta: &Meta, context: &[String]) -> i32 {
    match leading_context_overlap(meta, context) {
        0 => 0,
        count => 28 * count as i32,
    }
}

fn score_candidate(meta: &Meta, partial: Option<&str>, context: &[String], hint: Option<u8>) -> i32 {
    if is_punctuation_only(&meta.display) {
        return 0;
    }

    let partial_len = partial.map_or(0, str::len);
    let mut score = 0;

    if let Some(partial) = partial {
        if meta.norm.starts_with(partial) {
            score += 100;
        } else if meta.first.starts_with(partial) {
            score += 72;
        } else {
            return 0;
        }

        if meta.first == partial {
            score += 20;
        }
        if partial_len >= 3 {
            score += meta.norm.len().saturating_sub(partial_len) as i32 / 2;
        }
        if partial_len <= 1 && meta.display.len() <= 2 {
            score -= 18;
        }
        if partial_len <= 1 && matches!(hint, Some(dict) if dict != meta.dict) {
            score -= 10;
        }
    }

    let continuation = continuation_bonus(meta, context);
    let overlap = overlap_bonus(meta, context);
    let sequence = sequence_bonus(meta, context);
    let domain = if hint == Some(meta.dict) { 18 } else { 0 };

    if partial.is_none() && continuation == 0 && overlap == 0 && sequence == 0 && domain == 0 {
        return 0;
    }

    score += phrase_bonus(meta.word_count);
    score += continuation + overlap + sequence + domain;
    score.max(0)
}

fn sort_and_dedupe(suggester: &Suggester, mut scored: Vec<(i32, u16)>) -> Vec<String> {
    scored.sort_by(|a, b| {
        b.0.cmp(&a.0).then_with(|| {
            suggester.phrases[a.1 as usize]
                .display
                .cmp(&suggester.phrases[b.1 as usize].display)
        })
    });

    let mut seen = HashSet::with_capacity(MAX);
    let mut out = Vec::with_capacity(MAX);
    for (_, id) in scored {
        if out.len() >= MAX {
            break;
        }
        let phrase = &suggester.phrases[id as usize].display;
        if seen.insert(phrase.to_lowercase()) {
            out.push(phrase.clone());
        }
    }
    out
}

fn collect_context(prefix: &str, context: Option<String>) -> (String, Vec<String>) {
    let trailing_space = prefix.chars().last().is_some_and(char::is_whitespace);
    let normalized_prefix = short_message::normalize_text_for_autocomplete(prefix);

    let mut ctx = Vec::new();
    if let Some(raw_context) = context {
        let normalized_context = short_message::normalize_text_for_autocomplete(&raw_context);
        ctx.extend(normalized_context.split_whitespace().map(str::to_string));
    }

    let mut words: Vec<&str> = normalized_prefix.split_whitespace().collect();
    if trailing_space {
        ctx.extend(words.into_iter().map(str::to_string));
        return (String::new(), ctx);
    }

    let partial = words.pop().unwrap_or("").to_string();
    ctx.extend(words.into_iter().map(str::to_string));
    (partial, ctx)
}

#[tauri::command]
pub fn short_message_suggestions(prefix: String, context: Option<String>) -> Vec<String> {
    if prefix.trim().is_empty() {
        return Vec::new();
    }

    let (partial, context) = collect_context(&prefix, context);
    if partial.is_empty() && context.is_empty() {
        return Vec::new();
    }

    let suggester = suggester();
    let hint = domain_hint(suggester, &context);
    let phrase_ids = if partial.is_empty() {
        &suggester.root.subtree_ids
    } else {
        lookup_phrase_ids(&suggester.root, partial.as_bytes())
    };

    let scored: Vec<(i32, u16)> = phrase_ids
        .iter()
        .filter_map(|&id| {
            let score = score_candidate(
                &suggester.phrases[id as usize],
                (!partial.is_empty()).then_some(partial.as_str()),
                &context,
                hint,
            );
            (score > 0).then_some((score, id))
        })
        .collect();

    sort_and_dedupe(suggester, scored)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_prefix() {
        assert!(short_message_suggestions(String::new(), None).is_empty());
    }

    #[test]
    fn whitespace_prefix() {
        assert!(short_message_suggestions("   ".to_string(), None).is_empty());
    }

    #[test]
    fn prefix_completion() {
        let result = short_message_suggestions("pay".to_string(), None);
        assert!(!result.is_empty());
        assert!(result.len() <= MAX);
        assert!(result.iter().any(|item| item.to_lowercase().contains("pay")));
    }

    #[test]
    fn business_context() {
        let result = short_message_suggestions("inv".to_string(), Some("payment invoice balance".to_string()));
        assert!(!result.is_empty());
        assert!(result.iter().any(|item| item.to_lowercase().contains("invoice")));
    }

    #[test]
    fn dev_context() {
        let result = short_message_suggestions("build".to_string(), Some("code review deploy".to_string()));
        assert!(!result.is_empty());
        assert!(result.len() <= MAX);
    }

    #[test]
    fn social_context() {
        let result = short_message_suggestions("gm".to_string(), Some("gm anon".to_string()));
        assert!(!result.is_empty());
        assert!(result.len() <= MAX);
    }

    #[test]
    fn news_context() {
        let result = short_message_suggestions("market".to_string(), Some("news update launch market".to_string()));
        assert!(!result.is_empty());
        assert!(result.iter().any(|item| item.to_lowercase().contains("market")));
    }

    #[test]
    fn no_duplicates() {
        let result = short_message_suggestions("the".to_string(), None);
        let mut lowered: Vec<String> = result.iter().map(|item| item.to_lowercase()).collect();
        lowered.sort();
        let unique: HashSet<_> = lowered.iter().collect();
        assert_eq!(lowered.len(), unique.len());
    }

    #[test]
    fn bounded_count() {
        let result = short_message_suggestions("a".to_string(), None);
        assert!(result.len() <= MAX);
    }

    #[test]
    fn casing() {
        let result = short_message_suggestions("i think the".to_string(), None);
        for item in &result {
            assert!(!item.starts_with("i "), "lowercase 'i': {}", item);
        }
    }

    #[test]
    fn no_panic() {
        short_message_suggestions("test\x00foo".to_string(), None);
        short_message_suggestions("😂".to_string(), None);
        short_message_suggestions("test".to_string(), Some(String::new()));
        short_message_suggestions("a".repeat(500), None);
    }

    #[test]
    fn phrase_completion() {
        let result = short_message_suggestions("i".to_string(), Some("check cid on".to_string()));
        assert!(result.len() <= MAX);
    }

    #[test]
    fn next_word_respects_trailing_space() {
        let result = short_message_suggestions("payment ".to_string(), None);
        assert!(!result.is_empty());
        assert!(result.iter().any(|item| item.to_lowercase().starts_with("payment ")));
    }

    #[test]
    fn continuation_prefers_phrase_prefix_overlap() {
        let result = short_message_suggestions("check cid ".to_string(), None);
        assert!(!result.is_empty());
        assert!(result[0].to_lowercase().starts_with("cid "));
    }
}
