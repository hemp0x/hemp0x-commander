// Per-category public-style sample assembly. All output is synthetic.
// All samples stay under 120 characters.

use super::seed::SeedRng;
use super::vocab::*;
use std::fmt::Write;

fn ensure_under_120(s: &str) -> String {
    if s.chars().count() <= 120 {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(118).collect();
        out.push('…');
        out
    }
}

fn write_samples(out: &mut String, samples: Vec<String>) {
    for s in samples {
        let s = ensure_under_120(&s);
        let _ = writeln!(out, "{}", s);
    }
}

fn punct(rng: &mut SeedRng) -> &'static str {
    const PUNCT: &[&str] = &["", "!", ".", "?", "!!", "...", "!?", "?!"];
    rng.pick_str(PUNCT)
}

pub fn crypto_forum(rng: &mut SeedRng, count: usize) -> String {
    let mut out = String::new();
    let mut samples: Vec<String> = Vec::with_capacity(count);

    for _ in 0..count {
        let kind = rng.gen_range(0, 4);
        let s = match kind {
            0 => format!("{}{}", rng.pick(CRYPTO_HELP_OPENINGS), punct(rng)),
            1 => format!("{}{}", rng.pick(CRYPTO_HELP_REPLIES), punct(rng)),
            2 => format!("{}{}", rng.pick(CRYPTO_STATUS), punct(rng)),
            _ => format!("{}{}", rng.pick(CRYPTO_TECHNICAL), punct(rng)),
        };
        samples.push(s);
    }

    write_samples(&mut out, samples);
    out
}

pub fn sales_fulfillment(rng: &mut SeedRng, count: usize) -> String {
    let mut out = String::new();
    let mut samples: Vec<String> = Vec::with_capacity(count);

    let groups: &[&[&str]] = &[
        SALES_GREETING,
        SALES_ORDER,
        SALES_QUOTES,
        SALES_RETURNS,
        SALES_SUPPORT,
    ];

    for _ in 0..count {
        let group = rng.pick(groups);
        let s = format!("{}{}", rng.pick(group), punct(rng));
        samples.push(s);
    }

    write_samples(&mut out, samples);
    out
}

pub fn everyday_chat(rng: &mut SeedRng, count: usize) -> String {
    let mut out = String::new();
    let mut samples: Vec<String> = Vec::with_capacity(count);

    let groups: &[&[&str]] = &[
        EVERYDAY_GREETING,
        EVERYDAY_PLAN,
        EVERYDAY_UPDATE,
        EVERYDAY_FAMILY,
        EVERYDAY_QUESTION,
    ];

    for _ in 0..count {
        let group = rng.pick(groups);
        let e = rng.pick(EMOJI);
        let base = rng.pick(group);
        let p = punct(rng);
        let s = if rng.bool() {
            format!("{} {}{}", base, e, p)
        } else {
            format!("{}{}", base, p)
        };
        samples.push(s);
    }

    write_samples(&mut out, samples);
    out
}

pub fn ops_support(rng: &mut SeedRng, count: usize) -> String {
    let mut out = String::new();
    let mut samples: Vec<String> = Vec::with_capacity(count);

    let groups: &[&[&str]] = &[
        OPS_TICKET_OPEN,
        OPS_TICKET_UPDATE,
        OPS_INCIDENT,
        OPS_RESOLVE,
    ];

    for _ in 0..count {
        let group = rng.pick(groups);
        let s = format!("{}{}", rng.pick(group), punct(rng));
        samples.push(s);
    }

    write_samples(&mut out, samples);
    out
}

pub fn logistics_provenance(rng: &mut SeedRng, count: usize) -> String {
    let mut out = String::new();
    let mut samples: Vec<String> = Vec::with_capacity(count);

    let groups: &[&[&str]] = &[LOG_SHIPMENT, LOG_TRACE, LOG_COA, LOG_SUPPLIER];

    for _ in 0..count {
        let group = rng.pick(groups);
        let s = format!("{}{}", rng.pick(group), punct(rng));
        samples.push(s);
    }

    write_samples(&mut out, samples);
    out
}

pub fn edge_cases(rng: &mut SeedRng, count: usize) -> String {
    let mut out = String::new();
    let mut samples: Vec<String> = Vec::with_capacity(count);

    let groups: &[&[&str]] = &[EDGE_CID, EDGE_TXID, EDGE_URL, EDGE_PRICE, EDGE_DATE];

    for _ in 0..count {
        let group = rng.pick(groups);
        let s = format!("{}{}", rng.pick(group), punct(rng));
        samples.push(s);
    }

    write_samples(&mut out, samples);
    out
}
