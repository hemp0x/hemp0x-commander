// Shared codec tables live in their own module so the encode/decode logic
// stays compact and dictionary tuning remains isolated.
//
// Fallback alphabets:
// - 5-bit: compact lowercase/plain punctuation mode for dense simple text
// - 6-bit: broader ASCII-like symbol mode for mixed text, digits, and symbols
#[rustfmt::skip]
pub(crate) const ALPHABET_5BIT: [u8; 32] = [
    b' ', b'e', b't', b'a', b'o', b'i', b'n', b's', b'h', b'r', b'd', b'l', b'c', b'u', b'm',
    b'w', b'f', b'g', b'y', b'p', b'b', b'v', b'k', b'j', b'x', b'q', b'z', b'.', b'!', b'?',
    b',', b'-',
];

#[rustfmt::skip]
pub(crate) const ALPHABET_6BIT: [u8; 64] = [
    b' ', b'e', b't', b'a', b'o', b'i', b'n', b's', b'h', b'r', b'd', b'l', b'c', b'u', b'm',
    b'w', b'f', b'g', b'y', b'p', b'b', b'v', b'k', b'j', b'x', b'q', b'z', b'0', b'1', b'2',
    b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'.', b',', b'!', b'?', b'\'', b'-', b':', b'(',
    b')', b'@', b'#', b'/', b'+', b'"', b'&', b';', b'*', b'=', b'$', b'%', b'<', b'>', b'[',
    b']', b'_', b'`', b'~',
];

// DICT_A: primary general-purpose conversation and common blockchain-adjacent terms.
#[rustfmt::skip]
pub(crate) const DICT_A: [&str; 256] = [
    "the ", "and ", "to ", "of ", "a", "in ", "is ", "it", "you ", "that ",
    "for ", "on ", "are ", "with ", "as ", "be ", "at ", "by ", "not ", "this ",
    "have ", "from ", "or ", "had ", "but ", "what ", "all ", "were ", "when ",
    "we ", "your ", "can ", "said ", "there ", "an ", "if ", "will ", "up ", "out ",
    "so ", "some ", "her ", "would ", "make ", "like ", "him ", "into ", "time ",
    "has ", "look ", "more ", "see ", "way ", "could ", "people ", "my ", "than ",
    "first ", "been ", "who ", "now ", "find ", "long ", "down ", "day ", "did ",
    "get ", "come ", "made ", "may ", "part ", "over ", "new ", "take ", "only ",
    "just ", "really ", "thought ", "saw ", "think ", "know ", "want ", "going ",
    "about ", "here ", "they ", "them ", "their ", "then ", "please ", "thanks ",
    "good ", "bad ", "great ", "nice ", "love ", "hate ", "want to ", "going to ",
    "have to ", "need to ", "trying to ", "heard ", "watched ", "read ", "playing ",
    "working ", "waiting ", "coming ", "today ", "tonight ", "yesterday ",
    "tomorrow ", "already ", "finally ", "actually ", "probably ", "maybe ",
    "always ", "never ", "every ", "something ", "nothing ", "thing ",
    "stuff ", "idea ", "news ", "story ", "event ", "world ", "happened ",
    "dropped ", "launch ", "meme ", "lunch ", "break ", "fast ", "visit ",
    "listen ", "ing ", "ed ", "er ", "tion ", "ment ", "ness ", "ly ",
    "s ", "es ", "ies ", "re ", "un ", "dis ", "con ", "pre ", "pro ",
    "trans ", "he ", "the ", "her ", "their ", "theyre ", "its ", "with the ",
    "and the ", "but the ", "for the ", "on the ", "in the ", "to the ", "of the ",
    "this is ", "that is ", "what do you ", "how are you ", "i think ",
    "i saw ", "i heard ", "i want ", "i got ", "can you ", "did you ",
    "you know ", "crazy ", "insane ", "wild ", "fire ", "vibe ", "bet ",
    "dead ", "sent ", "seen ", "hemp ", "token ", "asset ", "sub ", "reward ",
    "payment ", "price of ", "cost of ", "right now ", "later ", "soon ",
    "complete ", "download ", "new ", " ", "!", "?", "¿", ".", ",", "...",
    "😂", "😭", "🔥", "💀", "❤️", "🙏", "👀", "🚀", "🫡", "😎", "🤣", "🥲",
    "😱", "🤯", "🙌", "💯", "✨", "->", "<-", "i ", "me ", "we ", "us ",
    "one ", "two ", "very ", "too ", "much ", "little ", "big ", "small ",
    "best ", "worst ", "happy ", "sad ", "funny ", "next ", "last ", "ever ",
    "once ", "again ", "back ", "away ", "home ",
];

// DICT_B: business, commerce, accounting, settlement, and transaction language.
#[rustfmt::skip]
pub(crate) const DICT_B: [&str; 256] = [
    "payment ", "payments ", "invoice ", "invoices ", "receipt ", "receipts ", "quote ", "quotes ",
    "order ", "orders ", "shipment ", "shipments ", "delivery ", "deliveries ", "purchase ", "purchases ",
    "sale ", "sales ", "merchant ", "merchants ", "customer ", "customers ", "client ", "clients ",
    "vendor ", "vendors ", "supplier ", "suppliers ", "contract ", "contracts ", "balance ", "balances ",
    "transfer ", "transfers ", "deposit ", "deposits ", "withdraw ", "withdrawal ", "withdrawals ", "payout ",
    "payouts ", "settlement ", "settlements ", "escrow ", "refund ", "refunds ", "rebate ", "rebates ",
    "fee ", "fees ", "tax ", "taxes ", "subtotal ", "total ", "amount ", "amount due ",
    "due date ", "overdue ", "paid ", "unpaid ", "pending ", "confirmed ", "failed ", "cancelled ",
    "approved ", "rejected ", "retry ", "urgent ", "asap ", "available balance ", "check balance ", "wallet ",
    "wallets ", "account ", "accounts ", "transaction ", "transactions ", "txid ", "ledger ", "statement ",
    "statements ", "billing ", "billing address ", "shipping ", "shipping address ", "tracking ", "tracking number ", "purchase order ",
    "sales order ", "invoice paid ", "payment sent ", "payment received ", "funds received ", "funds sent ", "cash flow ", "revenue ",
    "margin ", "profit ", "loss ", "budget ", "forecast ", "payroll ", "treasury ", "liquidity ",
    "capital ", "asset ", "assets ", "liability ", "liabilities ", "equity ", "audit ", "compliance ",
    "reconciled ", "reconciliation ", "subscribed ", "subscription ", "renewal ", "renewals ", "checkout ", "cart ",
    "sku ", "catalog ", "stock ", "inventory ", "warehouse ", "return ", "returns ", "exchange ",
    "bank transfer ", "wire transfer ", "ach ", "card payment ", "debit ", "credit ", "invoice total ", "payment method ",
    "reference number ", "order status ", "fulfillment ", "dispatched ", "delivered ", "delayed ", "partial refund ", "refund issued ",
    "payment link ", "checkout link ", "quote request ", "purchase request ", "support ", "contact support ", "sales team ", "finance team ",
    "accounting ", "bookkeeping ", "bookkeeper ", "balance due ", "net terms ", "net 30 ", "net 60 ", "discount ",
    "offer ", "promotion ", "coupon ", "coupon code ", "referral ", "referrals ", "commission ", "commissions ",
    "affiliate ", "affiliates ", "partner ", "partners ", "marketplace ", "storefront ", "shop ", "store ",
    "restock ", "preorder ", "waitlist ", "reservation ", "reserved ", "allocate ", "allocation ", "allocation complete ",
    "clearance ", "wholesale ", "retail ", "msrp ", "landed cost ", "unit cost ", "average cost ", "price update ",
    "price lock ", "premium ", "rebate approved ", "payout pending ", "settlement complete ", "under review ", "manual review ", "dispute ",
    "disputes ", "chargeback ", "chargebacks ", "dispute resolved ", "risk ", "fraud ", "verification ", "verified ",
    "kyc ", "aml ", "invoice number ", "po number ", "memo ", "note ", "notes ", "thank you ",
    "please ", "hello ", "hi ", "good morning ", "good afternoon ", "good evening ", "follow up ", "follow-up ",
    "status ", "update ", "updates ", "confirm receipt ", "send invoice ", "send payment ", "received payment ", "awaiting payment ",
    "close order ", "close ticket ", "open balance ", "open invoice ", "settlement window ", "clearing ", "cleared ", "treasury update ",
    "batch payout ", "batch settlement ", "multi sig ", "multisig ", "otc ", "desk ", "quote accepted ", "quote expired ",
    "payment complete ", "transfer complete ", "withdrawal complete ", "deposit complete ", "audit trail ", "invoice attached ", "remittance ", "remittance advice ",
];

// DICT_C: development, coding, frontend/backend, and technical workflow language.
#[rustfmt::skip]
pub(crate) const DICT_C: [&str; 256] = [
    "code ", "coding ", "developer ", "developers ", "development ", "dev environment ", "bug ", "bugs ",
    "fix ", "fixes ", "patch ", "patches ", "commit ", "commits ", "branch ", "branches ",
    "merge ", "merging ", "pull request ", "pr ", "review ", "reviewed ", "reviewing ", "build ",
    "builds ", "compile ", "compilation ", "deploy ", "deployment ", "release ", "version ", "versions ",
    "test ", "tests ", "unit test ", "integration test ", "regression test ", "e2e test ", "function ", "functions ",
    "method ", "methods ", "module ", "modules ", "component ", "components ", "state ", "store ",
    "props ", "hook ", "hooks ", "render ", "renderer ", "router ", "api ", "rpc ",
    "sdk ", "cli ", "json ", "schema ", "parser ", "encoder ", "decoder ", "serializer ",
    "deserialize ", "payload ", "request ", "response ", "endpoint ", "endpoints ", "websocket ", "stream ",
    "buffer ", "cache ", "cache hit ", "cache miss ", "memory ", "thread ", "async ", "await ",
    "future ", "promise ", "event loop ", "database ", "query ", "queries ", "index ", "indexes ",
    "migration ", "migrations ", "auth ", "login ", "logout ", "session ", "token ", "tokens ",
    "cookie ", "header ", "headers ", "body ", "frontend ", "backend ", "full stack ", "svelte ",
    "tauri ", "rust ", "cargo ", "crate ", "typescript ", "javascript ", "node ", "npm ",
    "vite ", "html ", "css ", "ui ", "ux ", "accessibility ", "responsive ", "mobile ",
    "desktop ", "layout ", "styling ", "design system ", "dark mode ", "light mode ", "clipboard ", "local storage ",
    "filesystem ", "fs ", "path ", "route ", "routes ", "middleware ", "service ", "services ",
    "controller ", "model ", "models ", "validation ", "sanitize ", "lint ", "formatter ", "format ",
    "hot reload ", "source map ", "stack trace ", "trace ", "debug ", "logging ", "logger ", "warning ",
    "warnings ", "error ", "errors ", "exception ", "panic ", "recover ", "fallback ", "retry ",
    "optimization ", "optimize ", "performance ", "profiling ", "benchmark ", "benchmarks ", "refactor ", "technical debt ",
    "dependency ", "dependencies ", "package ", "packages ", "library ", "libraries ", "git ", "github ",
    "ci ", "cd ", "pipeline ", "workflow ", "workflows ", "test suite ", "snapshot test ", "mock ",
    "mocks ", "fixture ", "fixtures ", "ref ", "refs ", "signal ", "signals ", "reducer ",
    "hydration ", "ssr ", "csr ", "wasm ", "web ", "browser ", "chrome ", "firefox ",
    "safari ", "edge ", "linux ", "macos ", "windows ", "build failed ", "pass tests ", "code review ",
    "merge conflict ", "fix forward ", "hot path ", "flaky test ", "deterministic ", "idempotent ", "concurrency ", "race condition ",
    "deadlock ", "throughput ", "latency ", "render pass ", "component state ", "feature flag ", "feature flags ", "rollback ",
    "rollout ", "ship it ", "done ", "todo ", "wip ", "draft ", "production ", "staging ",
    "local ", "remote ", "origin ", "upstream ", "review requested ", "approved ", "changes requested ", "resolved ",
    "checksum ", "crc ", "binary ", "frame ", "codec ", "compression ", "compressed ", "decompressed ",
    "alphabets ", "dictionary ", "dictionaries ", "runtime cache ", "no panic ", "validated ", "sanitized ", "stable ",
];

// DICT_D: social chat, memes, slang, and high-frequency community phrasing.
#[rustfmt::skip]
pub(crate) const DICT_D: [&str; 256] = [
    "lol ", "lmao ", "omg ", "wtf ", "fr ", "ngl ", "tbh ", "idk ",
    "imo ", "irl ", "mood ", "same ", "bet ", "fire ", "wild ", "crazy ",
    "insane ", "dead ", "sus ", "mid ", "vibe ", "vibes ", "bro ", "sis ",
    "fam ", "yo ", "nah ", "yeah ", "yep ", "nope ", "hype ", "goated ",
    "legendary ", "blessed ", "based ", "cringe ", "haha ", "hahaha ", "hehe ", "lolol ",
    "gm ", "gn ", "anon ", "ser ", "wen ", "alpha ", "cope ", "cope harder ",
    "send it ", "run it ", "no cap ", "for real ", "just saying ", "say less ", "check this out ", "watch this ",
    "look at this ", "you know ", "right now ", "later ", "soon ", "already ", "again ", "back ",
    "today ", "tonight ", "tomorrow ", "yesterday ", "weekend ", "😂", "😭", "🔥",
    "💀", "❤️", "🙏", "👀", "🚀", "morning ", "afternoon ", "evening ",
    "timeline ", "feed ", "post ", "posts ", "thread ", "threads ", "reply ", "replies ",
    "repost ", "reposted ", "dm ", "dms ", "group chat ", "channel ", "channels ", "notification ",
    "pfp ", "avatar ", "profile ", "bio ", "followers ", "following ", "mutuals ", "trend ",
    "trending ", "viral ", "meme ", "memes ", "joke ", "jokes ", "clip ", "clips ",
    "pic ", "pics ", "photo ", "photos ", "video ", "videos ", "stream ", "streaming ",
    "live ", "livestream ", "podcast ", "music ", "song ", "playlist ", "game ", "gaming ",
    "match ", "matches ", "win ", "wins ", "loss ", "losses ", "gg ", "ez ",
    "clutch ", "cooked ", "locked in ", "locked ", "unlocked ", "smooth ", "clean ", "messy ",
    "chaotic ", "wholesome ", "iconic ", "unreal ", "insane move ", "good call ", "bad call ", "hard fork ",
    "soft fork ", "token drop ", "airdrop ", "raid ", "shill ", "degen ", "aped ", "rugged ",
    "moon ", "mooning ", "rekt ", "pamp it ", "dump it ", "diamond hands ", "paper hands ", "bullish ",
    "bearish ", "green candle ", "red candle ", "pump ", "dump ", "breakout ", "correction ", "range ",
    "check the chart ", "check the wallet ", "on chain ", "off chain ", "mint ", "minted ", "burned ", "staking ",
    "unstaking ", "claim it ", "claim rewards ", "snapshot ", "reward drop ", "wallet ping ", "tx confirmed ", "cid posted ",
    "ipfs link ", "mint soon ", "launch soon ", "shipping soon ", "alpha soon ", "stay tuned ", "tap in ", "pull up ",
    "log in ", "log out ", "hold tight ", "buckle up ", "here we go ", "we are live ", "it is live ", "this hits ",
    "that hits ", "not bad ", "too good ", "so good ", "so bad ", "all in ", "max pain ", "low key ",
    "high key ", "full send ", "hard pass ", "soft launch ", "fast reply ", "slow reply ", "typing ", "read receipt ",
    "seen it ", "send meme ", "drop link ", "share link ", "pin this ", "save this ", "bookmark ", "copied ",
    "paste it ", "screenshot ", "screen recording ", "voice note ", "reaction ", "reacted ", "heart it ", "like it ",
    "dislike it ", "boost it ", "signal boost ", "community ", "spaces ", "room ", "rooms ", "hang tight ",
    "stay safe ", "good vibes ", "bad vibes ", "energy ", "aura ", "classic ", "instant classic ", "touch grass ",
];

// DICT_E: news, launches, market/events, governance, and chain-status language.
#[rustfmt::skip]
pub(crate) const DICT_E: [&str; 256] = [
    "news ", "update ", "updates ", "breaking ", "report ", "reports ", "story ", "stories ",
    "event ", "events ", "launch ", "launched ", "release ", "released ", "announced ", "announcement ",
    "market ", "markets ", "price ", "prices ", "volume ", "liquidity ", "volatility ", "trend ",
    "trends ", "today ", "tonight ", "tomorrow ", "yesterday ", "this week ", "this month ", "right now ",
    "protocol ", "network ", "chain ", "block ", "blocks ", "validator ", "validators ", "miner ",
    "miners ", "node ", "nodes ", "mempool ", "confirmation ", "confirmations ", "finality ", "throughput ",
    "upgrade ", "upgraded ", "fork ", "hard fork ", "soft fork ", "snapshot ", "snapshots ", "reward ",
    "rewards ", "holder ", "holders ", "token ", "tokens ", "asset ", "assets ", "qualifier ",
    "subasset ", "mint ", "minted ", "burn ", "burned ", "transfer ", "transferred ", "moved ",
    "dropped ", "surged ", "fell ", "gained ", "rallied ", "dipped ", "recovered ", "stabilized ",
    "announced today ", "announced tonight ", "now live ", "live now ", "going live ", "under way ", "confirmed ", "unconfirmed ",
    "complete ", "completed ", "delayed ", "resumed ", "paused ", "tracking ", "watching ", "monitoring ",
    "outlook ", "forecast ", "sentiment ", "adoption ", "usage ", "demand ", "supply ", "circulating supply ",
    "market cap ", "fdv ", "dominance ", "inflow ", "outflow ", "whale ", "whales ", "retail ",
    "institutions ", "institutional ", "treasury ", "governance ", "proposal ", "proposals ", "vote ", "voting ",
    "quorum ", "approved ", "rejected ", "passed ", "failed ", "mainnet ", "testnet ", "devnet ",
    "exploit ", "incident ", "outage ", "recovery ", "patch ", "patched ", "audit ", "audited ",
    "breach ", "fix deployed ", "response ", "postmortem ", "security ", "vulnerability ", "cve ", "mitigation ",
    "ecosystem ", "partner ", "partners ", "integration ", "integrated ", "listing ", "listed ", "delisting ",
    "exchange ", "exchanges ", "wallet support ", "rpc status ", "api status ", "dashboard ", "indexer ", "explorer ",
    "newsroom ", "press release ", "coverage ", "headline ", "headlines ", "reporter ", "commentary ", "analysis ",
    "briefing ", "bulletin ", "digest ", "recap ", "summary ", "highlights ", "alert ", "alerts ",
    "watchlist ", "calendar ", "agenda ", "keynote ", "session ", "panel ", "conference ", "summit ",
    "meetup ", "workshop ", "demo ", "showcase ", "premiere ", "rollout ", "migration ", "airdrop ",
    "distribution ", "rewards sent ", "snapshot taken ", "mint live ", "claim live ", "proposal live ", "trading live ", "governance live ",
    "launch window ", "maintenance ", "service restored ", "degraded performance ", "congestion ", "gas fees ", "fee spike ", "wallet activity ",
    "on chain data ", "off chain data ", "explorer link ", "cid update ", "ipfs update ", "hash ", "hashes ", "tx ",
    "txs ", "txid ", "txids ", "address ", "addresses ", "wallet ", "wallets ", "stake ",
    "staked ", "unstaked ", "delegate ", "delegated ", "validator set ", "epoch ", "slot ", "halving ",
    "emissions ", "burn rate ", "unlock ", "vesting ", "roadmap ", "milestone ", "milestones ", "patch notes ",
    "release notes ", "changelog ", "shipped ", "shipping ", "roadmap update ", "ecosystem update ", "market update ", "breaking news ",
    "new high ", "new low ", "all time high ", "all time low ", "green day ", "red day ", "watch closely ", "stay informed ",
];

// Dictionary selector mapping used by dictionary mode auto-selection:
// A = general, B = business, C = development, D = social, E = news/events.
#[rustfmt::skip]
pub(crate) const DICTIONARIES: [(&str, &[&str]); 5] = [
    ("A", &DICT_A),
    ("B", &DICT_B),
    ("C", &DICT_C),
    ("D", &DICT_D),
    ("E", &DICT_E),
];

#[rustfmt::skip]
pub(crate) const ACRONYMS: &[&str] = &[
    "api", "btw", "cid", "cli", "cpu", "db", "dev", "dht", "gpu", "gui", "http", "https", "hemp",
    "fr", "gm", "gn", "id", "idk", "imo", "ipfs", "irl", "json", "llm", "lol", "lmao", "nft", "ngl",
    "omg", "p2p", "pdf", "pr", "rpc", "sdk", "sql", "svg", "tbh", "ui", "url", "utxo", "ux", "tx",
    "txid", "ws", "wip", "wtf", "i",
];
