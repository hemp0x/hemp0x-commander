// Domain vocabularies for the public-style evidence corpus generator.

pub const CRYPTO_HELP_OPENINGS: &[&str] = &[
    "hey, my wallet is stuck syncing, any idea?",
    "is the mempool congested right now?",
    "fees are spiking, what's the recommended gwei?",
    "my tx has been pending for an hour, what do i do?",
    "how long does a confirmation usually take?",
    "the explorer shows my tx but my wallet says 0 conf",
    "i sent funds to a wrong address, is it recoverable?",
    "my seed phrase is rejected when i try to restore",
    "ledger not recognized on usb, tried 2 cables",
    "tx failed with insufficient funds but i have balance",
    "what does 'replace-by-fee' do exactly?",
    "any good pool recommendations for solo miners?",
    "is there a way to see pending nonce in the cli?",
    "the rpc endpoint keeps timing out, alternatives?",
    "wallet stuck on 'connecting to network' since this morning",
    "my balance dropped by 0.001 but i didn't send anything",
    "is the hemp0x testnet faucet up?",
    "how do i import a private key into a new wallet?",
    "do you use a hardware wallet for long term storage?",
    "which block explorers do you trust for hemp0x?",
];

pub const CRYPTO_HELP_REPLIES: &[&str] = &[
    "try restarting the daemon, that fixes most rpc issues",
    "fees are network-wide, try replacing with higher fee",
    "ledger not recognized -> try a different usb port and cable",
    "tx stays pending until a miner picks it up, no action needed",
    "your tx is in the mempool, you can rbf with higher fee",
    "check the explorer, if it shows up there it will confirm",
    "seed rejected usually means typo, double check each word",
    "wrong address -> sorry, transactions are not reversible",
    "use --rescan when restoring from seed to rebuild utxo set",
    "wait one more block, the chain is a bit slow today",
    "testnet faucet has been flaky, try the discord bot",
    "any address starting with the right prefix is valid",
    "no, the wallet doesn't need a daemon to receive funds",
    "yes, the cli shows pending nonce with `getmempoolinfo`",
    "increase the fee and use replace-by-fee, it usually works",
    "no kyc required for a non-custodial wallet",
    "the explorer is open source, you can self-host it",
    "the rpc timeout suggests your node is not synced yet",
    "your node is on the wrong chain, switch back to mainnet",
    "the mempool is full of low fee txs, give it a few hours",
];

pub const CRYPTO_STATUS: &[&str] = &[
    "tx confirmed in block 1000234, 6 confirmations",
    "tx 0xdeadbeef pending, low fee, may take a while",
    "tx 0xfeedface failed, nonce too low, please retry",
    "block 1000000 mined by pool hemp0x.example, reward 50",
    "block 1000001 mined, 3.2s avg, 142 tx in block",
    "height 1000500 reached, difficulty 1.42 gh/s",
    "hashrate at 1.4 th/s, 14 active workers",
    "hashrate dropped to 980 gh/s, 1 worker offline",
    "fee estimate: 12 sat/vB for 6 blocks, 4 for 60",
    "mempool has 12400 txs, mostly low-fee spam",
    "wallet balance updated after 1 confirmation",
    "wallet balance 0.142 hmp, last activity 2h ago",
    "address 0xabcd received 0.5 hmp from hemp1qxyz",
    "address verified, label set to 'cold storage'",
    "utxo set: 3 confirmed, 1 pending, 0 immature",
    "node status: synced, height 1000499, peers 12",
    "node status: catching up, 240 blocks behind",
    "node status: offline, last seen 2h ago",
    "rpc healthy, last call succeeded 30s ago",
    "rpc unhealthy, last call failed with timeout",
];

pub const CRYPTO_TECHNICAL: &[&str] = &[
    "swap eth for hmp on the dex, gas 0.001 eth",
    "bridge complete, hmp arrived on hemp0x mainnet",
    "airdrop claim open, snapshot at block 999000",
    "burn tx confirmed, total supply reduced by 1000",
    "mint tx confirmed, new nft collection live",
    "stake 100 hmp for 30 days, apy 5%",
    "unstake complete, principal returned to wallet",
    "withdraw pending, processing within 24h",
    "deposit confirmed, balance credited after 1 conf",
    "transfer tx sent, hash posted in #transactions",
    "kyc verified, withdrawal limits removed",
    "aml check cleared, no holds on this account",
    "fee waived for the first 1000 txs per address",
    "nonce 0 already used, try nonce 1",
    "utxo consolidation recommended, 14 small inputs",
    "wallet backup file: hemp0x-wallet-2026-05-30.dat",
    "private key never shared, stored in hardware wallet only",
    "two-factor enabled on the account, backup codes saved",
    "session expired, please re-authenticate",
    "api rate limit hit, retry after 60 seconds",
];

pub const SALES_GREETING: &[&str] = &[
    "thanks for reaching out, how can i help?",
    "thanks for your inquiry, the answer is yes",
    "thanks for the order, we will ship within 24h",
    "thanks for the payment, your invoice is now paid",
    "thanks for the patience, your case is being reviewed",
    "thanks for choosing us, we appreciate your business",
    "thanks for the quick reply, i'll check and get back to you",
    "thanks for the review, your feedback helps us improve",
    "thanks for the referral, your credit has been applied",
    "thanks for the tip, i'll pass it along to the team",
];

pub const SALES_ORDER: &[&str] = &[
    "your order has shipped, tracking number sent",
    "your order is being prepared, expected to ship tomorrow",
    "your order is ready for pickup at our warehouse",
    "your order has been delivered, please confirm receipt",
    "your order was returned, would you like a refund or exchange?",
    "your order was canceled per your request, refund issued",
    "your order is on backorder, expected restock in 2 weeks",
    "your order is partially shipped, two items pending",
    "your order is being held for address verification",
    "your order is delayed due to weather, sorry for the inconvenience",
];

pub const SALES_QUOTES: &[&str] = &[
    "quote for 100 units: $4,500, valid for 30 days",
    "quote attached, total includes shipping and tax",
    "quote revised per your feedback, total now $12,300",
    "quote approved, please send po to start the order",
    "quote pending your confirmation, expires 2026-07-15",
    "quote ready, let me know if you have any questions",
    "quote sent, please review and let me know if you need changes",
    "quote withdrawn as the item is now out of stock",
    "quote honored for 7 days from the issue date",
    "quote includes a 10% volume discount for orders over $5,000",
];

pub const SALES_RETURNS: &[&str] = &[
    "return label sent, please ship within 14 days",
    "return received, refund of $89.99 issued to original method",
    "return approved, please include the original packaging",
    "return denied, item shows signs of use beyond inspection",
    "return received damaged, photos attached for review",
    "return processed, store credit of $50 applied to your account",
    "return policy allows 30 days for unopened items",
    "return shipping is free for defective items",
    "return window closed, please contact support for exceptions",
    "return request received, we will reply within 1 business day",
];

pub const SALES_SUPPORT: &[&str] = &[
    "we received your message, ticket #1099 opened",
    "your support ticket is being escalated to engineering",
    "your warranty claim has been approved, replacement shipped",
    "your warranty claim requires a photo of the defect",
    "your product registration is complete, warranty valid for 2 years",
    "your product registration is incomplete, missing serial number",
    "your account is verified, all features are now available",
    "your account is locked due to too many failed login attempts",
    "your subscription has been renewed for another 12 months",
    "your subscription is set to cancel at the end of the billing cycle",
];

pub const EVERYDAY_GREETING: &[&str] = &[
    "hi mom, just checking in",
    "hey dad, how's it going?",
    "hi sis, did you get home ok?",
    "hey bro, what's up this weekend?",
    "hi grandma, happy birthday!",
    "hi uncle, thanks for the gift",
    "hi aunt, see you at thanksgiving",
    "hi cousin, long time no talk",
    "hi bestie, are we still on for friday?",
    "hi neighbor, sorry about the noise last night",
];

pub const EVERYDAY_PLAN: &[&str] = &[
    "are we still on for dinner tonight?",
    "want to grab coffee tomorrow morning?",
    "are you free this weekend for a hike?",
    "do you want to come over for the game?",
    "should we plan something for the holiday?",
    "what time should i pick you up?",
    "where do you want to meet?",
    "is 7pm too late for you?",
    "can we push it to next week instead?",
    "let's do saturday, sunday doesn't work for me",
];

pub const EVERYDAY_UPDATE: &[&str] = &[
    "running a few minutes late, sorry!",
    "running 10 min late, traffic is awful",
    "on my way, be there in 15",
    "just left, eta 20 min",
    "arrived, parking now",
    "heading home, see you in 30",
    "at the airport, flight on time",
    "flight delayed by an hour, sorry",
    "landed, baggage claim 4",
    "home safe, long day",
];

pub const EVERYDAY_FAMILY: &[&str] = &[
    "did you eat yet?",
    "don't forget to take your medicine",
    "love you, have a good day",
    "drive safe, text me when you get there",
    "call me when you can, no rush",
    "thinking of you, hope you're feeling better",
    "proud of you, big day today",
    "miss you, come visit soon",
    "see you at the weekend",
    "thanks for the call, meant a lot",
];

pub const EVERYDAY_QUESTION: &[&str] = &[
    "did you see the news today?",
    "have you tried that new restaurant?",
    "do you want me to pick anything up?",
    "are you feeling any better?",
    "did the package arrive yet?",
    "what's the wifi password again?",
    "is dad still napping?",
    "when's the next family dinner?",
    "where did you put the keys?",
    "how's the dog doing?",
];

pub const OPS_TICKET_OPEN: &[&str] = &[
    "ticket #1099 opened: cannot login to dashboard",
    "ticket #1100 opened: api returns 500 on /v1/orders",
    "ticket #1101 opened: mobile app crashes on launch",
    "ticket #1102 opened: payment double-charged",
    "ticket #1103 opened: account locked after password reset",
    "ticket #1104 opened: invoice missing tax line",
    "ticket #1105 opened: shipping label not generated",
    "ticket #1106 opened: webhook not firing for new orders",
    "ticket #1107 opened: timezone wrong on reports",
    "ticket #1108 opened: refund stuck in pending state",
];

pub const OPS_TICKET_UPDATE: &[&str] = &[
    "ticket #1099 updated: reproduced on staging, investigating",
    "ticket #1100 updated: root cause is null pointer in orders api",
    "ticket #1101 updated: hotfix released in v1.4.2",
    "ticket #1102 updated: second charge was a pre-auth, will clear",
    "ticket #1103 updated: account unlocked, password reset link sent",
    "ticket #1104 updated: tax calc fixed, please re-download pdf",
    "ticket #1105 updated: label provider was down, retry succeeded",
    "ticket #1106 updated: webhook secret rotated, please re-enter",
    "ticket #1107 updated: report timezone now uses user preference",
    "ticket #1108 updated: refund processed manually, please confirm",
];

pub const OPS_INCIDENT: &[&str] = &[
    "incident open: api latency above 5s for 15 minutes",
    "incident open: search service down, fallback to legacy",
    "incident open: database primary failed over to replica",
    "incident open: webhook delivery delayed by 30 minutes",
    "incident open: image upload returns 502 for files over 5mb",
    "incident open: signup form rejects valid emails",
    "incident open: email delivery delayed by 10 minutes",
    "incident open: s3 bucket returned 403 for new objects",
    "incident open: payment provider reports elevated error rate",
    "incident open: mobile push notifications stopped",
];

pub const OPS_RESOLVE: &[&str] = &[
    "incident resolved: api latency back to normal at 14:30",
    "incident resolved: search service restored, no data loss",
    "incident resolved: database primary back online, replica demoted",
    "incident resolved: webhook delivery caught up at 14:45",
    "incident resolved: image upload now works for files up to 50mb",
    "incident resolved: signup form fixed, tested with 50 emails",
    "incident resolved: email backlog cleared at 15:10",
    "incident resolved: s3 permissions fixed, bucket public-read restored",
    "incident resolved: payment provider recovered, error rate at 0.1%",
    "incident resolved: push notifications restored for ios and android",
];

pub const LOG_SHIPMENT: &[&str] = &[
    "shipment SH-1001 received at warehouse 4",
    "shipment SH-1002 dispatched from origin facility",
    "shipment SH-1003 in transit, eta 2026-06-10",
    "shipment SH-1004 out for delivery, driver 23",
    "shipment SH-1005 delivered, signed by receptionist",
    "shipment SH-1006 returned to sender per customer request",
    "shipment SH-1007 damaged in transit, claim opened",
    "shipment SH-1008 lost in transit, investigation pending",
    "shipment SH-1009 on hold at customs, paperwork needed",
    "shipment SH-1010 cleared customs, on final leg",
];

pub const LOG_TRACE: &[&str] = &[
    "lot L-2026-0420 harvested 2026-05-30, weight 12.4 kg",
    "lot L-2026-0420 sent for lab testing, coa pending",
    "lot L-2026-0420 passed lab, thc 18.2%, cbd 0.4%",
    "lot L-2026-0420 packaged on 2026-06-02, batch B-0042",
    "lot L-2026-0420 shipped to dispensary DC-007, 50 units",
    "lot L-2026-0420 received at dispensary, scanned in",
    "lot L-2026-0420 sold through, 38 units remaining",
    "lot L-2026-0420 expired 2026-12-31, removed from shelf",
    "lot L-2026-0420 destroyed per regulation, certificate filed",
    "lot L-2026-0420 recalled, customer notification sent",
];

pub const LOG_COA: &[&str] = &[
    "coa attached: thc 18.2%, cbd 0.4%, cbn 0.1%",
    "coa verified by independent lab, results within spec",
    "coa failed: pesticide levels above allowed limit",
    "coa failed: heavy metals above allowed limit",
    "coa passed microbial testing, ready for distribution",
    "coa pending lab analysis, results in 5-7 business days",
    "coa on file, valid through 2027-06-30",
    "coa requested, please send latest version",
    "coa signed and uploaded to compliance system",
    "coa requires signature, please review and sign",
];

pub const LOG_SUPPLIER: &[&str] = &[
    "supplier confirmed delivery for 2026-06-15",
    "supplier delayed, new eta 2026-06-20",
    "supplier shipment received, 100 units short",
    "supplier shipment received, all units accounted for",
    "supplier invoice received, payment due in 30 days",
    "supplier payment sent, please confirm receipt",
    "supplier quality issue reported, replacement requested",
    "supplier contact updated, new rep is alice",
    "supplier contract renewed for another 12 months",
    "supplier contract expired, sourcing new vendor",
];

pub const EDGE_CID: &[&str] = &[
    "cid bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi",
    "cid QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG",
    "cid bafybeibhpxl3pvvsvcyckwxeh2vkh7pkb6yt54q3lkg5h5lcmwi7t6s6ba",
    "ipfs cid posted in #announcements",
    "ipfs file updated, check the latest",
    "ipfs link: ipfs.io/ipfs/bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi",
    "ipfs metadata: name, image, attributes, all on chain",
    "ipfs gateway slow, try a different mirror",
    "ipfs pinned to 3 nodes, redundancy ok",
    "ipfs unpinned, content may disappear in 24h",
];

pub const EDGE_TXID: &[&str] = &[
    "tx 0xdeadbeef confirmed at block 1000234",
    "tx 0xfeedface pending, low fee",
    "tx 0x1234567890abcdef1234567890abcdef12345678 failed",
    "tx 0xabcdef1234567890abcdef1234567890abcdef12 in mempool",
    "txid posted: 0x1234...cdef, please confirm",
    "txid received, thank you, verifying now",
    "txid not found on the explorer, are you sure?",
    "txid looks malformed, please double check",
    "txid verified, 3 confirmations, all good",
    "txid: 0x0000...0000, that's an invalid tx, sorry",
];

pub const EDGE_URL: &[&str] = &[
    "see the docs: docs.example.com/short-messages",
    "see https://example.com/path?query=1 for details",
    "check example.com/help for the latest faq",
    "docs at github.com/hemp0x/commander",
    "blog post: blog.example.com/2026/06/short-message-update",
    "whitepaper: whitepaper.example.com/hemp0x-v1.pdf",
    "audit: github.com/hemp0x/commander/security",
    "release notes: github.com/hemp0x/commander/releases",
    "guide: guide.example.com/getting-started",
    "api reference: api.example.com/v1",
];

pub const EDGE_PRICE: &[&str] = &[
    "price $10.00",
    "price $25.50",
    "price $49.99",
    "price $99.99",
    "price $199.99",
    "price $499.00",
    "price $1,000.00",
    "price $1,250.75",
    "price $5,000.00",
    "price $10,000.00",
];

pub const EDGE_DATE: &[&str] = &[
    "date 2026-01-01",
    "date 2026-02-14",
    "date 2026-03-31",
    "date 2026-04-15",
    "date 2026-05-30",
    "date 2026-06-04",
    "date 2026-07-04",
    "date 2026-08-15",
    "date 2026-09-30",
    "date 2026-12-25",
];

pub const EMOJI: &[&str] = &[
    "😀", "😃", "😄", "😁", "😆", "😅", "😂", "🤣", "😊", "😇", "🙂", "🙃", "😉", "😌", "😍", "🥰",
    "😘", "😗", "😙", "😚", "😋", "😛", "😜", "🤪", "😝", "🤑", "🤗", "🤭", "🤫", "🤔", "😎", "🤓",
    "🧐", "😕", "😟", "🙁", "☹️", "😮", "😯", "😲", "😳", "🥺", "😦", "😧", "😨", "😰", "😥", "😢",
    "😭", "😱", "😖", "😣", "😞", "😓", "😩", "😫", "🥱", "😤", "😡", "😠", "🤬", "😈", "👿", "💀",
    "☠️", "💩", "🤡", "👻", "👽", "👾", "🤖", "😺", "😸", "😹", "😻", "😼", "😽", "🙀", "😿", "😾",
    "💋", "💌", "💘", "💝", "💖", "💗", "💓", "💞", "💕", "💟", "❣️", "💔", "❤️", "🧡", "💛", "💚",
    "💙", "💜", "🤎", "🖤", "🤍", "💯", "💢", "💥", "💫", "💦", "💨", "💬", "💭", "💤", "👋", "🤚",
    "🖐️", "✋", "🖖", "👌", "🤌", "🤏", "✌️", "🤞", "🤟", "🤘", "🤙", "👈", "👉", "👆", "🖕", "👇",
    "☝️", "👍", "👎", "✊", "👊", "🤛", "🤜", "👏", "🙌", "👐", "🤲", "🤝", "🙏", "✍️", "💪", "🦾",
    "🦵", "🦶", "👂", "🦻", "👃", "🧠", "👀", "👁️", "👅", "👄", "🩸", "🔥", "✨", "🌟", "⭐", "☀️",
    "🌤️", "⛅", "🌥️", "☁️", "🌦️", "🌧️", "⛈️", "🌩️", "🌨️", "❄️", "☃️", "⛄", "🌬️", "💧", "☔", "☂️",
    "🌊", "🌫️", "🌪️", "🌈", "🌂", "🚀", "🛸",
];
