// gen_short_message_corpus
//
// One-shot corpus generator for the short-message audit tooling.
//
// Emits deterministic plain-text files under
//   untracked/commander-v1.4/short-message-corpus/
// one message per line, with `#` comments at the top of each file. Re-running
// this binary with no args overwrites the same files; the output is fully
// deterministic so future audits can be reproduced exactly.

use std::fs;
use std::io::Write;
use std::path::PathBuf;

mod emit;
mod seedrng;
mod vocab;

fn out_dir() -> PathBuf {
    PathBuf::from("untracked/commander-v1.4/short-message-corpus")
}

fn write_file(path: &PathBuf, header: &[&str], body: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut f = fs::File::create(path)?;
    for h in header {
        writeln!(f, "# {}", h)?;
    }
    writeln!(f)?;
    f.write_all(body.as_bytes())?;
    Ok(())
}

fn main() {
    let dir = out_dir();
    fs::create_dir_all(&dir).expect("create corpus dir");

    let mut rng = seedrng::SeedRng::new(0xA17_B17A_C0DE);

    let casual = emit::casual_chat(&mut rng, 2200);
    let business = emit::business_commerce(&mut rng, 2200);
    let operations = emit::operations_support(&mut rng, 2200);
    let asset = emit::asset_holder(&mut rng, 1700);
    let logistics = emit::logistics_trace(&mut rng, 1700);
    let crypto = emit::crypto_hemp0x(&mut rng, 1700);
    let edge = emit::edge_cases(&mut rng, 1700);
    let mixed = emit::mixed_realistic(&mut rng, 1500);

    write_file(
        &dir.join("casual-chat.txt"),
        &[
            "casual-chat.txt",
            "Casual chat, greetings, social coordination.",
            "One message per line. Lines starting with # are comments.",
            "Generated deterministically by gen_short_message_corpus.",
        ],
        &casual,
    )
    .expect("write casual-chat.txt");
    write_file(
        &dir.join("business-commerce.txt"),
        &[
            "business-commerce.txt",
            "Business, commerce, payment, invoice, and order messages.",
            "One message per line. Lines starting with # are comments.",
            "Generated deterministically by gen_short_message_corpus.",
        ],
        &business,
    )
    .expect("write business-commerce.txt");
    write_file(
        &dir.join("operations-support.txt"),
        &[
            "operations-support.txt",
            "Operations, project, support, status, and team communication.",
            "One message per line. Lines starting with # are comments.",
            "Generated deterministically by gen_short_message_corpus.",
        ],
        &operations,
    )
    .expect("write operations-support.txt");
    write_file(
        &dir.join("asset-holder-ipfs-nft.txt"),
        &[
            "asset-holder-ipfs-nft.txt",
            "Asset owner, holder, IPFS, NFT, collection, and announcement messages.",
            "One message per line. Lines starting with # are comments.",
            "Generated deterministically by gen_short_message_corpus.",
        ],
        &asset,
    )
    .expect("write asset-holder-ipfs-nft.txt");
    write_file(
        &dir.join("logistics-traceability.txt"),
        &[
            "logistics-traceability.txt",
            "Logistics, traceability, provenance, custody, and shipment messages.",
            "One message per line. Lines starting with # are comments.",
            "Generated deterministically by gen_short_message_corpus.",
        ],
        &logistics,
    )
    .expect("write logistics-traceability.txt");
    write_file(
        &dir.join("crypto-hemp0x-wallet.txt"),
        &[
            "crypto-hemp0x-wallet.txt",
            "Crypto, Hemp0x, wallet, mining, on-chain, and tech messages.",
            "One message per line. Lines starting with # are comments.",
            "Generated deterministically by gen_short_message_corpus.",
        ],
        &crypto,
    )
    .expect("write crypto-hemp0x-wallet.txt");
    write_file(
        &dir.join("edge-cases.txt"),
        &[
            "edge-cases.txt",
            "Edge cases: punctuation, emojis, mixed numbers, dates, prices,",
            "order IDs, txid-like, CID-like, phone/time formats, urls, emails.",
            "Generated deterministically by gen_short_message_corpus.",
        ],
        &edge,
    )
    .expect("write edge-cases.txt");
    write_file(
        &dir.join("mixed-realistic.txt"),
        &[
            "mixed-realistic.txt",
            "Realistic cross-category short messages that mix multiple topics.",
            "Useful for catching tokenizer leaks across dictionaries.",
            "Generated deterministically by gen_short_message_corpus.",
        ],
        &mixed,
    )
    .expect("write mixed-realistic.txt");

    let total = casual.lines().count()
        + business.lines().count()
        + operations.lines().count()
        + asset.lines().count()
        + logistics.lines().count()
        + crypto.lines().count()
        + edge.lines().count()
        + mixed.lines().count();
    println!(
        "wrote corpus files to {} ({} samples)",
        dir.display(),
        total
    );
}
