// Synthetic short-message corpus generators, one per category. Each
// generator returns a String containing one sample per line. Samples are
// synthetic but realistic and intentionally vary punctuation, casing, and
// numbers so the audit tool sees natural-looking traffic.

use super::seedrng::SeedRng;
use super::vocab::*;
use std::fmt::Write;

fn pick_n<'a>(rng: &mut SeedRng, src: &'a [&'a str], n: usize) -> Vec<&'a str> {
    let mut out: Vec<&'a str> = Vec::with_capacity(n);
    let mut used: Vec<usize> = Vec::with_capacity(n);
    for _ in 0..n {
        let mut idx;
        let mut guard = 0;
        loop {
            idx = rng.gen_range(0, src.len());
            if !used.contains(&idx) || guard > 16 {
                break;
            }
            guard += 1;
        }
        used.push(idx);
        out.push(src[idx]);
    }
    out
}

fn punct(rng: &mut SeedRng) -> &'static str {
    rng.maybe_punct()
}

fn capitalize(s: &str) -> String {
    SeedRng::cap_first(s)
}

fn ensure_under_120(s: &str) -> String {
    if s.len() <= 120 {
        s.to_string()
    } else {
        s.chars().take(118).collect::<String>() + "…"
    }
}

fn write_samples(out: &mut String, samples: Vec<String>) {
    for s in samples {
        let s = ensure_under_120(&s);
        let _ = writeln!(out, "{}", s);
    }
}

pub fn casual_chat(rng: &mut SeedRng, count: usize) -> String {
    let mut out = String::new();
    let mut samples: Vec<String> = Vec::with_capacity(count);

    for _ in 0..count / 6 {
        let g = rng.pick(CASUAL_GREETINGS);
        let e = rng.pick(EMOJI);
        let p = punct(rng);
        let body = if e.is_empty() {
            format!("{}{}", capitalize(g), p)
        } else {
            format!("{} {}{}", capitalize(g), e, p)
        };
        samples.push(body);

        let q = rng.pick(CASUAL_QUESTIONS);
        samples.push(format!("{}{}", capitalize(q), punct(rng)));

        let pl = rng.pick(CASUAL_PLEASANTRIES);
        samples.push(format!("{}{}", capitalize(pl), punct(rng)));

        let a = rng.pick(CASUAL_ACK);
        let e = rng.pick(EMOJI);
        if e.is_empty() {
            samples.push(format!("{}{}", capitalize(a), punct(rng)));
        } else {
            samples.push(format!("{} {}{}", capitalize(a), e, punct(rng)));
        }

        let c = rng.pick(CASUAL_CASUAL);
        let e = rng.pick(EMOJI);
        let p = punct(rng);
        if e.is_empty() {
            samples.push(format!("{}{}", capitalize(c), p));
        } else {
            samples.push(format!("{} {}{}", c, e, p));
        }

        let co = rng.pick(CASUAL_COORD);
        samples.push(format!("{}{}", capitalize(co), punct(rng)));
    }

    while samples.len() < count {
        let g = rng.pick(CASUAL_GREETINGS);
        let e = rng.pick(EMOJI);
        let p = punct(rng);
        let body = if e.is_empty() {
            format!("{}{}", capitalize(g), p)
        } else {
            format!("{} {}{}", capitalize(g), e, p)
        };
        samples.push(body);
    }
    samples.truncate(count);
    write_samples(&mut out, samples);
    out
}

pub fn business_commerce(rng: &mut SeedRng, count: usize) -> String {
    let mut out = String::new();
    let mut samples: Vec<String> = Vec::with_capacity(count);

    let invoice = |n: u32| -> String { format!("invoice {}", n) };
    let order = |n: u32| -> String { format!("order {}", n) };
    let receipt = |n: u32| -> String { format!("receipt {}", n) };
    let sku = |n: u32| -> String { format!("sku {}", n) };
    let payment = |n: u32| -> String { format!("payment {}", n) };
    let po = |n: u32| -> String { format!("po {}", n) };
    let refn = |n: u32| -> String { format!("ref {}", n) };

    let statuses: &[&str] = &[
        "paid",
        "pending",
        "received",
        "sent",
        "overdue",
        "due",
        "complete",
        "confirmed",
    ];
    let actions: &[&str] = &[
        "please confirm",
        "please review",
        "please verify",
        "please sign",
    ];
    let amounts: &[&str] = &[
        "$10.00",
        "$25.50",
        "$49.99",
        "$100.00",
        "$199.99",
        "$250.00",
        "$499.00",
        "$1,000.00",
        "$1,250.75",
        "$2,500.00",
        "$5,000.00",
        "$10,000.00",
        "$0.50",
        "$1.00",
        "$5.00",
        "$0.01",
        "$0.10",
    ];

    let mut seq: u32 = 1000;
    for _ in 0..count {
        let kind = rng.gen_range(0, 14);
        seq = seq.wrapping_add(rng.gen_range(1, 3) as u32);
        let s = match kind {
            0 => format!("{} {}", invoice(seq), rng.pick(statuses)),
            1 => format!("{} {}", order(seq), rng.pick(statuses)),
            2 => format!("{} {}", receipt(seq), rng.pick(statuses)),
            3 => format!("{} {}", sku(seq), rng.pick(EMOJI)),
            4 => format!("{} {}", payment(seq), rng.pick(EMOJI)),
            5 => format!("{} {}", po(seq), rng.pick(statuses)),
            6 => format!("{} {}", refn(seq), rng.pick(statuses)),
            7 => format!("{} received {}", invoice(seq), rng.pick(amounts)),
            8 => format!(
                "{} due {} {}",
                invoice(seq),
                rng.pick(MONTHS),
                rng.gen_range(1, 28)
            ),
            9 => format!(
                "{} {} for {}",
                order(seq),
                rng.pick(statuses),
                rng.pick(amounts)
            ),
            10 => format!("{} {}", rng.pick(actions), invoice(seq)),
            11 => format!("{} {}", rng.pick(actions), order(seq)),
            12 => format!("{} {}", rng.pick(actions), payment(seq)),
            _ => format!(
                "{} {} {}",
                invoice(seq),
                rng.pick(statuses),
                rng.pick(amounts)
            ),
        };
        samples.push(s);
    }

    write_samples(&mut out, samples);
    out
}

pub fn operations_support(rng: &mut SeedRng, count: usize) -> String {
    let mut out = String::new();
    let mut samples: Vec<String> = Vec::with_capacity(count);

    let actions: &[&str] = &[
        "task",
        "ticket",
        "issue",
        "bug",
        "fix",
        "build",
        "deploy",
        "release",
        "patch",
        "hotfix",
        "rollback",
        "incident",
        "report",
        "review",
        "approval",
        "request",
        "follow up",
        "check-in",
        "sync",
        "standup",
        "retro",
        "demo",
        "kickoff",
    ];
    let statuses: &[&str] = &[
        "open",
        "closed",
        "pending",
        "in progress",
        "blocked",
        "complete",
        "completed",
        "done",
        "cancelled",
        "canceled",
        "deferred",
        "on hold",
        "in review",
        "in qa",
        "in testing",
        "in design",
        "in development",
        "ready for review",
        "ready for qa",
        "ready for release",
        "approved",
        "rejected",
        "received",
        "sent",
        "verified",
    ];
    let priorities: &[&str] = &["low", "medium", "high", "urgent", "p1", "p2", "p3", "p4"];
    let platforms: &[&str] = &[
        "ios", "android", "web", "desktop", "api", "backend", "frontend", "infra",
    ];
    let teams: &[&str] = &[
        "core",
        "platform",
        "growth",
        "design",
        "qa",
        "ops",
        "support",
        "sales",
        "marketing",
    ];

    for _ in 0..count {
        let kind = rng.gen_range(0, 12);
        let s = match kind {
            0 => format!("{} {}", rng.pick(actions), rng.pick(statuses)),
            1 => format!("{} {} priority", rng.pick(actions), rng.pick(priorities)),
            2 => format!("{} assigned to {}", rng.pick(actions), rng.pick(teams)),
            3 => format!("{} merged to {}", rng.pick(actions), rng.pick(platforms)),
            4 => format!(
                "{} ready for review on {}",
                rng.pick(actions),
                rng.pick(platforms)
            ),
            5 => format!("{} blocked on {}", rng.pick(actions), rng.pick(teams)),
            6 => format!("{} failed on {}", rng.pick(actions), rng.pick(platforms)),
            7 => format!("{} passed on {}", rng.pick(actions), rng.pick(platforms)),
            8 => format!("{} reopened on {}", rng.pick(actions), rng.pick(platforms)),
            9 => format!("{} for sprint {}", rng.pick(actions), rng.gen_range(1, 30)),
            10 => format!("{} update from {}", rng.pick(actions), rng.pick(teams)),
            _ => format!(
                "{} {} on {}",
                rng.pick(actions),
                rng.pick(statuses),
                rng.pick(platforms)
            ),
        };
        samples.push(s);
    }

    write_samples(&mut out, samples);
    out
}

pub fn asset_holder(rng: &mut SeedRng, count: usize) -> String {
    let mut out = String::new();
    let mut samples: Vec<String> = Vec::with_capacity(count);

    let events: &[&str] = &[
        "ipfs file",
        "ipfs update",
        "ipfs metadata",
        "ipfs cid",
        "ipfs link",
        "nft drop",
        "nft mint",
        "nft claim",
        "nft reveal",
        "nft metadata",
        "nft edition",
        "nft announcement",
        "nft update",
        "nft airdrop",
        "new collection",
        "new drop",
        "new mint",
        "new nft",
        "new edition",
        "collection live",
        "collection update",
        "collection metadata",
        "holder notice",
        "holder update",
        "holder message",
        "all holders",
        "asset holder",
        "asset update",
        "asset metadata",
        "asset posted",
        "asset available",
        "asset claim",
        "asset file",
        "asset document",
        "owner update",
        "owner message",
        "owner notice",
        "owner announcement",
        "update available",
        "update posted",
        "update ready",
        "update live",
        "update sent",
        "latest update",
        "latest file",
        "latest document",
        "latest notice",
        "latest announcement",
        "new file",
        "new document",
        "new attachment",
        "new notice",
        "new announcement",
        "file available",
        "file posted",
        "file updated",
        "file sent",
        "file received",
        "document available",
        "document posted",
        "document updated",
        "document sent",
        "document received",
        "details available",
        "details posted",
        "details updated",
        "check latest",
        "read more",
        "proof attached",
        "public notice",
        "public update",
        "official update",
        "official announcement",
        "important notice",
        "important update",
        "big update",
        "community notice",
        "community update",
        "member update",
        "metadata posted",
        "artwork posted",
        "artwork available",
        "image attached",
        "release ready",
        "release posted",
        "reveal ready",
        "drop live",
        "drop posted",
        "mint open",
        "claim open",
        "holder only",
        "stay tuned",
        "we are live",
    ];
    let modifiers: &[&str] = &[
        "",
        " now",
        " today",
        " soon",
        " is live",
        " is open",
        " is closed",
        " is ready",
        " is pending",
        " is complete",
    ];

    for _ in 0..count {
        let kind = rng.gen_range(0, 8);
        let s = match kind {
            0 => format!("{}{}", rng.pick(events), rng.pick(modifiers)),
            1 => format!("{} {}", rng.pick(events), rng.pick(modifiers)),
            2 => {
                let id = rng.gen_range(1, 9999);
                format!("asset #{} posted", id)
            }
            3 => {
                let id = rng.gen_range(1, 9999);
                format!("collection #{} live", id)
            }
            4 => {
                let id = rng.gen_range(1, 999);
                format!("nft #{} minted", id)
            }
            5 => {
                let id = rng.gen_range(1, 999);
                format!("nft #{} claimed", id)
            }
            6 => {
                let id = rng.gen_range(1, 999);
                format!("edition #{} announced", id)
            }
            _ => {
                let id = rng.gen_range(1, 9999);
                format!("asset #{} updated", id)
            }
        };
        samples.push(s);
    }

    write_samples(&mut out, samples);
    out
}

pub fn logistics_trace(rng: &mut SeedRng, count: usize) -> String {
    let mut out = String::new();
    let mut samples: Vec<String> = Vec::with_capacity(count);

    let stages: &[&str] = &[
        "shipment",
        "package",
        "parcel",
        "delivery",
        "pickup",
        "dispatch",
        "carrier",
        "courier",
        "freight",
        "container",
        "pallet",
        "manifest",
        "tracking",
        "warehouse",
        "hub",
        "facility",
        "inventory",
        "stock",
        "seal",
        "scan",
        "origin",
        "destination",
        "inbound",
        "outbound",
        "return",
        "batch",
        "lot",
        "serial",
        "label",
        "route",
        "load",
        "release",
        "receive",
    ];
    let statuses: &[&str] = &[
        "received",
        "pending",
        "in transit",
        "delivered",
        "delayed",
        "dispatched",
        "sent",
        "in route",
        "arrived",
        "scheduled",
        "ready",
        "complete",
        "cancelled",
        "canceled",
        "verified",
        "checked",
        "printed",
        "applied",
        "broken",
        "low",
        "out",
        "released to",
        "ready for",
        "available for",
        "scanned at",
    ];
    let locations: &[&str] = &[
        "hub",
        "warehouse",
        "facility",
        "port",
        "dock",
        "origin",
        "destination",
        "sort center",
        "depot",
        "transit",
        "carrier",
        "customer",
        "client",
    ];
    let modifiers: &[&str] = &["", " today", " now", " soon", " on time", " early", " late"];

    for _ in 0..count {
        let kind = rng.gen_range(0, 11);
        let s = match kind {
            0 => format!("{} {}", rng.pick(stages), rng.pick(statuses)),
            1 => {
                let id = rng.gen_range(1000, 999999);
                format!("{} #{} {}", rng.pick(stages), id, rng.pick(statuses))
            }
            2 => format!(
                "{} {} at {}",
                rng.pick(stages),
                rng.pick(statuses),
                rng.pick(locations)
            ),
            3 => {
                let id = rng.gen_range(1000, 999999);
                format!("tracking #{} {}", id, rng.pick(statuses))
            }
            4 => {
                let id = rng.gen_range(1, 100);
                format!("batch #{} {}", id, rng.pick(statuses))
            }
            5 => {
                let id = rng.gen_range(1, 1000);
                format!("lot #{} {}", id, rng.pick(statuses))
            }
            6 => {
                let id = rng.gen_range(1, 99999);
                format!("serial #{} {}", id, rng.pick(statuses))
            }
            7 => {
                let w = rng.gen_range(1, 1000);
                format!("weight {}kg {}", w, rng.pick(statuses))
            }
            8 => {
                let q = rng.gen_range(1, 1000);
                format!("qty {} {}", q, rng.pick(statuses))
            }
            9 => format!(
                "{} {}{}",
                rng.pick(stages),
                rng.pick(statuses),
                rng.pick(modifiers)
            ),
            _ => {
                let id = rng.gen_range(1, 1000);
                format!(
                    "{} #{} {} {}",
                    rng.pick(stages),
                    id,
                    rng.pick(statuses),
                    rng.pick(locations)
                )
            }
        };
        samples.push(s);
    }

    write_samples(&mut out, samples);
    out
}

pub fn crypto_hemp0x(rng: &mut SeedRng, count: usize) -> String {
    let mut out = String::new();
    let mut samples: Vec<String> = Vec::with_capacity(count);

    let nouns: &[&str] = &[
        "wallet",
        "node",
        "pool",
        "miner",
        "block",
        "tx",
        "txid",
        "hash",
        "nonce",
        "fee",
        "fees",
        "gas",
        "rpc",
        "api",
        "endpoint",
        "explorer",
        "balance",
        "stake",
        "swap",
        "bridge",
        "airdrop",
        "burn",
        "mint",
        "transfer",
        "transaction",
        "block reward",
        "difficulty",
        "hashrate",
        "mainnet",
        "testnet",
        "address",
        "key",
        "seed",
        "ledger",
    ];
    let verbs_states: &[&str] = &[
        "ready",
        "pending",
        "confirmed",
        "failed",
        "online",
        "offline",
        "synced",
        "updating",
        "updated",
        "broadcast",
        "broadcasting",
        "verified",
        "received",
        "sent",
        "accepted",
        "rejected",
        "stuck",
        "queued",
        "open",
        "closed",
        "low",
        "high",
        "stable",
    ];
    let tokens: &[&str] = &[
        "hemp", "btc", "eth", "usdt", "usdc", "dai", "hemp0x", "wbtc", "weth",
    ];
    let chains: &[&str] = &[
        "mainnet", "testnet", "sepolia", "holesky", "polygon", "arbitrum", "optimism",
    ];
    let statuses_extra: &[&str] = &[
        "mempool",
        "in mempool",
        "out of mempool",
        "replaced",
        "dropped",
        "expired",
    ];

    let hex_chars = [
        '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
    ];
    let mut hex_buf = String::with_capacity(64);
    for _ in 0..count {
        let kind = rng.gen_range(0, 13);
        let s = match kind {
            0 => format!("{} {}", rng.pick(nouns), rng.pick(verbs_states)),
            1 => format!(
                "{} {} on {}",
                rng.pick(nouns),
                rng.pick(verbs_states),
                rng.pick(chains)
            ),
            2 => format!("{} transfer {}", rng.pick(tokens), rng.pick(verbs_states)),
            3 => {
                let n = rng.gen_range(1, 100);
                format!(
                    "{} {} confirmed in {} blocks",
                    rng.pick(tokens),
                    rng.pick(verbs_states),
                    n
                )
            }
            4 => {
                let id = rng.gen_range(1, 999999) as u32;
                format!("block #{} {}", id, rng.pick(verbs_states))
            }
            5 => {
                let h = rng.gen_range(1, 1000000) as u32;
                format!("height {} {}", h, rng.pick(verbs_states))
            }
            6 => {
                let d = rng.gen_range(1, 1000000) as u32;
                format!("difficulty {} {}", d, rng.pick(verbs_states))
            }
            7 => {
                let h = rng.gen_range(1, 1000000) as u32;
                format!("hashrate {} {}", h, rng.pick(verbs_states))
            }
            8 => {
                hex_buf.clear();
                let len = rng.gen_range(8, 40);
                for _ in 0..len {
                    hex_buf.push(hex_chars[rng.gen_range(0, hex_chars.len())]);
                }
                format!("0x{} {}", hex_buf, rng.pick(statuses_extra))
            }
            9 => {
                let amt = rng.gen_range(1, 1000) as f64 / 100.0;
                format!("{} {} {}", rng.pick(tokens), amt, rng.pick(verbs_states))
            }
            10 => {
                let n = rng.gen_range(1, 100);
                format!("confirmations {} {}", n, rng.pick(verbs_states))
            }
            11 => {
                let a = rng.gen_range(1, 10000);
                let b = rng.gen_range(1, 10000);
                format!("block {} of {} {}", a, b, rng.pick(verbs_states))
            }
            _ => {
                let s = rng.gen_range(1, 100);
                let e = rng.gen_range(1, 100);
                format!(
                    "{} of {} {} {}",
                    s,
                    e,
                    rng.pick(tokens),
                    rng.pick(verbs_states)
                )
            }
        };
        samples.push(s);
    }

    write_samples(&mut out, samples);
    out
}

pub fn edge_cases(rng: &mut SeedRng, count: usize) -> String {
    let mut out = String::new();
    let mut samples: Vec<String> = Vec::with_capacity(count);

    let currencies: &[&str] = &[
        "$1.00",
        "$5.00",
        "$10.00",
        "$19.99",
        "$25.50",
        "$49.99",
        "$99.99",
        "$100.00",
        "$199.99",
        "$250.00",
        "$499.00",
        "$999.99",
        "$1,000.00",
        "$1,250.75",
        "$2,500.00",
        "$5,000.00",
        "$9,999.99",
        "$10,000.00",
        "$0.01",
        "$0.10",
        "$0.50",
        "€10.00",
        "€100.00",
        "€1,000.00",
        "£10.00",
        "£100.00",
        "£1,000.00",
        "¥100",
        "¥1,000",
        "¥10,000",
    ];
    let dates: &[&str] = &[
        "2026-01-01",
        "2026-01-10",
        "2026-01-15",
        "2026-01-31",
        "2026-02-01",
        "2026-02-14",
        "2026-02-28",
        "2026-03-01",
        "2026-03-15",
        "2026-03-31",
        "2026-04-01",
        "2026-04-15",
        "2026-04-30",
        "2026-05-01",
        "2026-05-15",
        "2026-05-31",
        "2026-06-01",
        "2026-06-04",
        "2026-06-15",
        "2026-06-30",
        "2026-07-01",
        "2026-07-15",
        "2026-07-31",
        "2026-08-01",
        "2026-08-15",
        "2026-08-31",
        "2026-09-01",
        "2026-09-15",
        "2026-09-30",
        "2026-10-01",
        "2026-10-15",
        "2026-10-31",
        "2026-11-01",
        "2026-11-15",
        "2026-11-30",
        "2026-12-01",
        "2026-12-15",
        "2026-12-25",
        "2026-12-31",
    ];
    let times: &[&str] = TIME_HOURS;
    let phones: &[&str] = &[
        "+1-555-555-5555",
        "+44 20 7946 0958",
        "+49 30 12345678",
        "+81 3-1234-5678",
        "+61 2 9876 5432",
        "+33 1 23 45 67 89",
        "+34 91 123 45 67",
        "+39 06 1234 5678",
        "+52 55 1234 5678",
        "+55 11 91234-5678",
        "+91 22 1234 5678",
        "+86 10 1234 5678",
        "+7 495 123-45-67",
        "+82 2-1234-5678",
        "+972 3-123-4567",
        "+971 4-123-4567",
    ];
    let cids: &[&str] = &[
        "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi",
        "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG",
        "QmTzQ1Nj5an1bY5Y2FzM7HkBGEzCkjL6mX7S7p9P9p9P9p9",
        "bafybeibhpxl3pvvsvcyckwxeh2vkh7pkb6yt54q3lkg5h5lcmwi7t6s6ba",
        "QmbWqxBEKC3Q8ywhnX2FQ4f7X5d4K3R8P3K9N5T2Y1B2C4D",
        "bafybeif2pall7dybz7v4w4vuw4w7y5y4p4p4p4p4p4p4p4p4p4p4p4p4p",
    ];
    let hex_chars = [
        '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
    ];

    let mut hex_buf = String::with_capacity(64);
    let mut hex_buf2 = String::with_capacity(64);

    for _ in 0..count {
        let kind = rng.gen_range(0, 28);
        let s = match kind {
            0 => format!("invoice {} {}", rng.gen_range(1000, 9999), rng.pick(dates)),
            1 => format!("order {} {}", rng.gen_range(1000, 9999), rng.pick(dates)),
            2 => format!("receipt {} {}", rng.gen_range(1000, 9999), rng.pick(dates)),
            3 => format!("payment {} on {}", rng.pick(currencies), rng.pick(dates)),
            4 => format!("amount {} due {}", rng.pick(currencies), rng.pick(dates)),
            5 => format!("{} at {}", rng.pick(dates), rng.pick(times)),
            6 => format!("phone {}", rng.pick(phones)),
            7 => format!("call {} at {}", rng.pick(phones), rng.pick(times)),
            8 => {
                hex_buf.clear();
                for _ in 0..40 {
                    hex_buf.push(hex_chars[rng.gen_range(0, hex_chars.len())]);
                }
                format!("addr 0x{}", hex_buf)
            }
            9 => {
                hex_buf.clear();
                for _ in 0..40 {
                    hex_buf.push(hex_chars[rng.gen_range(0, hex_chars.len())]);
                }
                format!("address 0x{}", hex_buf)
            }
            10 => {
                hex_buf.clear();
                hex_buf2.clear();
                for _ in 0..16 {
                    hex_buf.push(hex_chars[rng.gen_range(0, hex_chars.len())]);
                }
                for _ in 0..16 {
                    hex_buf2.push(hex_chars[rng.gen_range(0, hex_chars.len())]);
                }
                format!("tx 0x{} -> 0x{}", hex_buf, hex_buf2)
            }
            11 => {
                hex_buf.clear();
                for _ in 0..16 {
                    hex_buf.push(hex_chars[rng.gen_range(0, hex_chars.len())]);
                }
                format!("txid 0x{}", hex_buf)
            }
            12 => {
                hex_buf.clear();
                for _ in 0..40 {
                    hex_buf.push(hex_chars[rng.gen_range(0, hex_chars.len())]);
                }
                format!("hash 0x{}", hex_buf)
            }
            13 => format!("cid {}", rng.pick(cids)),
            14 => format!("ipfs cid {}", rng.pick(cids)),
            15 => {
                let d = rng.pick(CITIES);
                let t = rng.pick(TIME_HOURS);
                format!("meeting at {} {}", d, t)
            }
            16 => {
                let d = rng.pick(CITIES);
                let t = rng.pick(TIME_HOURS);
                format!("call at {} {}", d, t)
            }
            17 => format!("hello {}!", rng.pick(EMOJI)),
            18 => format!("gm {}", rng.pick(EMOJI)),
            19 => format!("gn {}", rng.pick(EMOJI)),
            20 => format!("thanks {}", rng.pick(EMOJI)),
            21 => format!("lol {}", rng.pick(EMOJI)),
            22 => format!("{}{}", rng.pick(EMOJI), rng.pick(EMOJI)),
            23 => format!("{}!", rng.pick(EMOJI)),
            24 => {
                let n1 = rng.gen_range(1, 100);
                let n2 = rng.gen_range(1, 100);
                format!("v{}.{}.{}", n1, n2, rng.gen_range(0, 20))
            }
            25 => {
                let major = rng.gen_range(1, 5);
                let minor = rng.gen_range(0, 10);
                let patch = rng.gen_range(0, 30);
                format!("v{}.{}.{} release", major, minor, patch)
            }
            26 => format!("build #{}", rng.gen_range(100, 999999)),
            27 => {
                let city = rng.pick(CITIES);
                let name = rng.pick(FIRST_NAMES);
                let e = rng.pick(EMOJI);
                format!("{} is in {} {}", capitalize(name), capitalize(city), e)
            }
            _ => format!("hello {} {}", rng.pick(FIRST_NAMES), rng.pick(EMOJI)),
        };
        samples.push(s);
    }

    write_samples(&mut out, samples);
    out
}

pub fn mixed_realistic(rng: &mut SeedRng, count: usize) -> String {
    let mut out = String::new();
    let mut samples: Vec<String> = Vec::with_capacity(count);

    let templates: &[(&str, &[&str])] = &[
        (
            "gm {}, {} today",
            &[
                "wonderful",
                "great",
                "fun",
                "busy",
                "wild",
                "crazy",
                "productive",
            ],
        ),
        (
            "hi {}, can you check the {} for me?",
            &["alice", "bob", "carol", "dave", "erin", "frank"],
        ),
        (
            "hey {}, the {} is ready for review",
            &["alice", "bob", "carol", "dave", "erin", "frank"],
        ),
        (
            "{} — please review when you get a chance",
            &[
                "invoice paid",
                "order confirmed",
                "shipment received",
                "task complete",
            ],
        ),
        (
            "{} — let me know if you have questions",
            &[
                "invoice paid",
                "order confirmed",
                "shipment received",
                "task complete",
            ],
        ),
        (
            "thanks for the {} {}",
            &["update", "info", "help", "support", "patience"],
        ),
        (
            "{} {} sent, please confirm",
            &["invoice", "order", "receipt", "shipment", "tracking"],
        ),
        (
            "{} {} received, thank you",
            &["invoice", "order", "receipt", "shipment", "tracking"],
        ),
        (
            "wallet {} {} confirmed",
            &["transfer", "deposit", "withdrawal", "payment"],
        ),
        (
            "tx {} {} at {}",
            &["0x1234", "0xabcd", "0xdeadbeef", "0xfeedface"],
        ),
        (
            "block {} {} at height {}",
            &["0x1234", "0xabcd", "0xdeadbeef"],
        ),
        ("{} at {} {} for {}", &["meeting", "call", "review", "demo"]),
        (
            "see you {}, drive safe!",
            &["later", "soon", "tomorrow", "tonight"],
        ),
        (
            "have a good {} {}",
            &["day", "night", "morning", "evening", "weekend"],
        ),
        (
            "let me know when you {}",
            &[
                "arrive", "leave", "finish", "start", "ship", "send", "receive",
            ],
        ),
    ];

    for _ in 0..count {
        let (template, fillers) = rng.pick(templates);
        let n_fillers = template.matches("{}").count();
        let picks = pick_n(rng, fillers, n_fillers);
        let mut s = template.to_string();
        for p in picks {
            if let Some(start) = s.find("{}") {
                s = format!("{}{}{}", &s[..start], p, &s[start + 2..]);
            }
        }
        let e = rng.pick(EMOJI);
        if !e.is_empty() && rng.bool() {
            s.push(' ');
            s.push_str(e);
        }
        let p = punct(rng);
        if !p.is_empty() {
            s.push_str(p);
        }
        samples.push(s);
    }

    write_samples(&mut out, samples);
    out
}
