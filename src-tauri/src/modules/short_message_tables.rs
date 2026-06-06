// HOXSHTV1.0 Hemp0x Short Hand Tables Version 1.0
//
// HOXSHT is a short-hand message table format for the Hemp0x protocol.
// It is designed to be compact and efficient for use on-chain.
//
// Wire format summary:
// - Every Commander short message is a fixed 32-byte frame.
// - Dictionary mode stores one byte per dictionary token, plus compact literal
//   escapes for unknown ASCII, long digit runs, and number-like runs.
// - The frame header stores a 3-bit dictionary index, so DICTIONARIES may hold
//   at most 8 dictionaries unless FORMAT_VERSION changes.
// - Token 255 is reserved for dictionary literal escape. Fixed-size
//   dictionaries should keep entry 255 as ""; variable-size dictionaries must
//   stay below 255 real entries.
// - After public on-chain use begins, reordering, deleting, or changing token
//   meanings requires a new FORMAT_VERSION. Add-only changes are also unsafe
//   for fixed-size dictionaries if they shift token indexes.
//
// Dictionary tuning rules:
// - Normal full words and phrases usually end in a trailing space:
//   "payment ", "thank you ", "start ".
// - Punctuation, symbols, emojis, URL fragments, and stems usually do not:
//   "!", "?", ".com", "pric", "ship".
// - Suffix tokens that finish a word usually include a trailing space:
//   "ed ", "ing ", "e ", "ment ". The codec can attach these to a preceding
//   trailing-space stem by removing the stem's space. For example:
//   "start " + "ed " => "started "; "pric " + "ing " => "pricing ".
// - Keep very common complete words as one token when possible. Stem+suffix
//   costs two tokens, so it is best for widening coverage across many variants.
// - Keep the standalone " " token. It is useful when composing around stems,
//   literals, and words that are not already stored with a trailing space.
//
// Fallback alphabets:
// - 5-bit: compact lowercase/plain punctuation mode for dense simple text.
// - 6-bit: broader ASCII-like symbol mode for mixed text, digits, and symbols.
// Public version marker. Kept as a `pub` constant so external tooling
// (e.g. the corpus audit binary) can identify which table pack is compiled
// in. The full table fingerprint is computed by `table_identity` below.
pub const HOXSHT_VERSION_MARKER: &str = "HOXSHTV1.0";

#[rustfmt::skip]
pub const ALPHABET_5BIT: [u8; 32] = [
    b' ', b'e', b't', b'a', b'o', b'i', b'n', b's', b'h', b'r', b'd', b'l', b'c', b'u', b'm',
    b'w', b'f', b'g', b'y', b'p', b'b', b'v', b'k', b'j', b'x', b'q', b'z', b'.', b'!', b'?',
    b',', b'-',
];

#[rustfmt::skip]
pub const ALPHABET_6BIT: [u8; 64] = [
    b' ', b'e', b't', b'a', b'o', b'i', b'n', b's', b'h', b'r', b'd', b'l', b'c', b'u', b'm',
    b'w', b'f', b'g', b'y', b'p', b'b', b'v', b'k', b'j', b'x', b'q', b'z', b'0', b'1', b'2',
    b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'.', b',', b'!', b'?', b'\'', b'-', b':', b'(',
    b')', b'@', b'#', b'/', b'+', b'"', b'&', b';', b'*', b'=', b'$', b'%', b'<', b'>', b'[',
    b']', b'_', b'`', b'~',
];

// Suffixes are normal dictionary strings with one extra decode rule: when a
// suffix token follows output ending in a space, the decoder removes that space
// before appending the suffix. The encoder also uses this list to split words
// into stem+suffix pairs where both tokens exist in the selected dictionary.
#[rustfmt::skip]
pub const SUFFIXES: &[&str] = &[
    "'s ", "able ", "al ", "ation ", "d ", "e ", "ed ", "er ", "ers ", "es ", "est ", "ful ",
    "ial ", "ible ", "ies ", "ing ", "ion ", "ions ", "ish ", "ized ", "less ", "ly ", "ment ",
    "ness ", "ping ", "s ", "ting ", "tion ", "tions ", "y ",
];

// Index 255 is reserved for the dictionary literal escape and must stay empty.

// DICT_A = General conversation base 1
// DICT_B = General conversation base 2
// DICT_C = Operations / coordination
// DICT_D = Commerce / finance
// DICT_E = General phrase pack
// DICT_F = Asset-owner / holder / announcement phrase pack
// DICT_G = Traceability / logistics / provenance
// DICT_H = Crypto / Hemp0x / on-chain / tech

// DICT_A: primary general-purpose conversation.
#[rustfmt::skip]
pub const DICT_A: [&str; 256] = [
    " ", "!", ",", ".", "...", "?", "😎", "🙏",
    "🔥", "😂", "❤️", "🚀", "'s ", "able ", "al ", "ation ",
    "d ", "e ", "ed ", "er ", "ers ", "es ", "est ", "ful ",
    "ies ", "ing ", "ion ", "ions ", "less ", "ly ", "ment ", "ness ",
    "s ", "tion ", "tions ", "y ", "a ", "about ", "after ", "again ",
    "all ", "already ", "am ", "and ", "any ", "are ", "as ", "ask ",
    "at ", "back ", "be ", "because ", "been ", "before ", "best ", "better ",
    "but ", "by ", "can ", "come ", "could ", "day ", "did ", "do ",
    "does ", "done ", "down ", "feel ", "find ", "for ", "from ", "get ",
    "give ", "go ", "going ", "good ", "got ", "great ", "guess ", "had ",
    "happen ", "happy ", "has ", "have ", "hear ", "help ", "here ", "hope ",
    "how ", "i ", "if ", "in ", "into ", "is ", "it ", "its ",
    "just ", "keep ", "know ", "last ", "later ", "let ", "like ", "little ",
    "long ", "look ", "made ", "make ", "maybe ", "me ", "mean ", "message ",
    "might ", "more ", "much ", "my ", "need ", "never ", "new ", "next ",
    "nice ", "no ", "not ", "nothing ", "now ", "of ", "ok ", "on ",
    "one ", "or ", "other ", "out ", "people ", "please ", "really ", "right ",
    "say ", "see ", "send ", "sent ", "so ", "some ", "someone ", "something ",
    "soon ", "sorry ", "sound ", "start ", "still ", "sure ", "take ", "talk ",
    "tell ", "than ", "thank ", "thanks ", "that ", "the ", "their ", "them ",
    "then ", "there ", "they ", "thing ", "think ", "this ", "thought ", "through ",
    "time ", "to ", "today ", "tomorrow ", "tonight ", "too ", "try ", "trying ",
    "understand ", "up ", "us ", "use ", "very ", "wait ", "want ", "way ",
    "we ", "well ", "were ", "what ", "when ", "where ", "which ", "who ",
    "why ", "will ", "with ", "work ", "working ", "would ", "wrong ", "yes ",
    "yesterday ", "you ", "your ", "check ", "call ", "text ", "weather ", "hi ",
    "how are you ", "i got ", "i heard ", "i saw ", "i think ", "i want ", "i will ", "i need ",
    "let me know ", "no problem ", "sounds good ", "talk soon ", "what do you ", "you know ", "want to ", "going to ",
    "need to ", "trying to ", "this is ", "good morning ", "good night ", "on my way ", "see you ", "all good ",
    "not sure ", "right now ", "thank you ", "good idea ", "sounds like ", "gm ", "gn ", "lol ",
    "meme ", "vibe ", "wild ", "crazy ", "insane ", "👀", "💀", "💯",
    "😭", "😱", "🙌", "🤣", "🤯", "🥲", "🫡", "",
];

// DICT_B: secondary general-purpose conversation, life, travel, health, weather, and practical status.
#[rustfmt::skip]
pub const DICT_B: [&str; 256] = [
    " ", "!", ",", ".", "...", "?", "😎", "🙏",
    "🔥", "😂", "❤️", "'s ", "able ", "al ", "ation ", "d ",
    "e ", "ed ", "er ", "ers ", "es ", "est ", "ful ", "ies ",
    "ing ", "ion ", "ions ", "less ", "ly ", "ment ", "ness ", "s ",
    "tion ", "tions ", "y ", "a ", "about ", "after ", "again ", "all ",
    "already ", "am ", "and ", "answer ", "any ", "are ", "as ", "ask ",
    "at ", "away ", "be ", "because ", "been ", "before ", "better ", "between ",
    "but ", "by ", "can ", "could ", "did ", "do ", "does ", "done ",
    "during ", "for ", "friend ", "from ", "get ", "give ", "go ", "going ",
    "good ", "got ", "had ", "has ", "have ", "help ", "here ", "home ",
    "hope ", "hospital ", "house ", "how ", "i ", "if ", "in ", "into ",
    "is ", "it ", "just ", "know ", "later ", "left ", "let ", "make ",
    "maybe ", "me ", "medicine ", "more ", "my ", "near ", "need ", "new ",
    "next ", "no ", "not ", "now ", "of ", "off ", "ok ", "on ",
    "or ", "other ", "out ", "outside ", "please ", "question ", "ready ", "right ",
    "send ", "sent ", "should ", "soon ", "still ", "sure ", "take ", "than ",
    "thank ", "thank you ", "thanks ", "that ", "the ", "their ", "them ", "then ",
    "there ", "they ", "this ", "through ", "time ", "to ", "today ", "tomorrow ",
    "under ", "until ", "up ", "us ", "use ", "very ", "want ", "was ",
    "way ", "we ", "were ", "when ", "where ", "which ", "while ", "who ",
    "why ", "will ", "with ", "without ", "would ", "yes ", "you ", "your ",
    "address ", "air ", "around ", "arriv ", "back ", "bad ", "battery ", "bring ",
    "call ", "car ", "care ", "chang ", "check ", "city ", "cold ", "come ",
    "day ", "delay ", "doctor ", "down ", "drive ", "eat ", "emergency ", "feel ",
    "fever ", "find ", "finish ", "fix ", "food ", "gas ", "hour ", "late ",
    "leave ", "life ", "locat ", "lost ", "meet ", "message ", "minute ", "move ",
    "night ", "pain ", "phone ", "place ", "plan ", "problem ", "rain ", "rest ",
    "ride ", "road ", "safe ", "see ", "sick ", "sleep ", "snow ", "start ",
    "stop ", "store ", "talk ", "tell ", "think ", "tired ", "trip ", "try ",
    "turn ", "wait ", "walk ", "warm ", "watch ", "water ", "weather ", "week ",
    "work ", "working ", "wrong ", "all good ", "good morning ", "good night ", "how are you ", "let me know ",
    "no problem ", "not sure ", "on my way ", "right now ", "see you ", "sounds good ", "talk soon ", "",
];

// DICT_C: operations, coordination, project, status, support, and team communication.
#[rustfmt::skip]
pub const DICT_C: [&str; 256] = [
    " ", "!", ",", ".", "=", "?", "$", "%",
    "😎", "🙏", "'s ", "able ", "al ", "ation ", "d ", "e ",
    "ed ", "er ", "ers ", "es ", "est ", "ful ", "ies ", "ing ",
    "ion ", "ions ", "less ", "ly ", "ment ", "ness ", "s ", "tion ",
    "tions ", "y ", "a ", "about ", "after ", "again ", "all ", "already ",
    "am ", "and ", "any ", "are ", "around ", "as ", "ask ", "at ",
    "be ", "because ", "been ", "before ", "between ", "but ", "by ", "can ",
    "could ", "did ", "do ", "does ", "done ", "during ", "for ", "from ",
    "get ", "give ", "good ", "had ", "has ", "have ", "help ", "here ",
    "how ", "i ", "if ", "in ", "into ", "is ", "it ", "just ",
    "know ", "later ", "let ", "let me know ", "make ", "me ", "more ", "my ",
    "need ", "new ", "next ", "no ", "not ", "now ", "of ", "ok ",
    "on ", "or ", "other ", "please ", "ready ", "send ", "sent ", "should ",
    "since ", "soon ", "still ", "sure ", "than ", "thank ", "thank you ", "thanks ",
    "that ", "the ", "their ", "them ", "then ", "there ", "they ", "this ",
    "through ", "time ", "to ", "today ", "tomorrow ", "until ", "us ", "use ",
    "want ", "way ", "we ", "week ", "when ", "where ", "which ", "while ",
    "why ", "will ", "with ", "without ", "work ", "would ", "yes ", "you ",
    "your ", "action ", "active ", "add ", "address ", "adjust ", "alert ", "approv ",
    "assign ", "avail ", "back ", "backup ", "block ", "build ", "call ", "cancel ",
    "chang ", "check ", "client ", "clos ", "complete ", "confirm ", "contact ", "correct ",
    "creat ", "customer ", "date ", "day ", "delay ", "deliver ", "detail ", "develop ",
    "document ", "email ", "entry ", "error ", "estimat ", "expect ", "fail ", "feedback ",
    "file ", "final ", "find ", "finish ", "fix ", "follow ", "forward ", "goal ",
    "hold ", "hour ", "important ", "inform ", "issue ", "item ", "job ", "lead ",
    "link ", "list ", "location ", "look ", "manage ", "meet ", "message ", "minute ",
    "move ", "note ", "notice ", "open ", "order ", "owner ", "partner ", "pending ",
    "plan ", "problem ", "process ", "progress ", "project ", "provid ", "question ", "receiv ",
    "release ", "reply ", "report ", "request ", "requir ", "resolv ", "response ", "review ",
    "schedul ", "service ", "setting ", "share ", "show ", "solv ", "start ", "status ",
    "step ", "submit ", "support ", "task ", "team ", "ticket ", "track ", "troubleshoot ",
    "understand ", "updat ", "urgent ", "vendor ", "verify ", "wait ", "working ", "",
];

// DICT_D: commerce, finance, payment, order, and settlement language.
#[rustfmt::skip]
pub const DICT_D: [&str; 256] = [
    " ", "!", ",", ".", "=", "?", "$", "%",
    "😎", "🙏", "'s ", "able ", "al ", "ation ", "d ", "e ",
    "ed ", "er ", "ers ", "es ", "est ", "ful ", "ies ", "ing ",
    "ion ", "ions ", "less ", "ly ", "ment ", "ness ", "s ", "tion ",
    "tions ", "y ", "a ", "about ", "after ", "again ", "all ", "already ",
    "am ", "and ", "any ", "are ", "ask ", "at ", "be ", "because ",
    "been ", "before ", "between ", "but ", "by ", "can ", "could ", "did ",
    "do ", "does ", "done ", "during ", "for ", "from ", "get ", "give ",
    "good ", "had ", "has ", "have ", "help ", "here ", "how ", "i ",
    "if ", "in ", "into ", "is ", "it ", "just ", "know ", "later ",
    "let me know ", "make ", "me ", "more ", "my ", "need ", "new ", "next ",
    "no ", "not ", "now ", "of ", "ok ", "on ", "or ", "other ",
    "please ", "ready ", "send ", "sent ", "should ", "since ", "soon ", "still ",
    "sure ", "than ", "thank ", "thank you ", "thanks ", "that ", "the ", "their ",
    "them ", "then ", "there ", "they ", "this ", "through ", "time ", "to ",
    "today ", "tomorrow ", "until ", "us ", "use ", "want ", "we ", "week ",
    "when ", "where ", "which ", "while ", "why ", "will ", "with ", "without ",
    "work ", "would ", "yes ", "you ", "your ", "account ", "address ", "amount ",
    "approv ", "asset ", "attach ", "avail ", "balance ", "bank ", "batch ", "bill ",
    "billing ", "budget ", "business ", "buy ", "card ", "cart ", "cash ", "charg ",
    "check ", "checkout ", "client ", "complete ", "confirm ", "contact ", "contract ", "cost ",
    "coupon ", "credit ", "customer ", "date ", "day ", "debit ", "deposit ", "detail ",
    "discount ", "dispute ", "document ", "due ", "email ", "entry ", "estimat ", "expect ",
    "expense ", "fee ", "finance ", "fund ", "invoice ", "item ", "link ", "manage ",
    "margin ", "market ", "method ", "net ", "note ", "number ", "offer ", "open ",
    "order ", "overdue ", "owner ", "paid ", "partner ", "pay ", "payment ", "payout ",
    "pending ", "price ", "process ", "product ", "profit ", "purchase ", "quote ", "receiv ",
    "receipt ", "refund ", "return ", "revenue ", "sale ", "sell ", "service ", "settle ",
    "shop ", "statement ", "status ", "stock ", "store ", "supplier ", "supply ", "support ",
    "tax ", "ticket ", "total ", "transaction ", "transfer ", "transferr ", "unit ", "unpaid ",
    "update ", "urgent ", "vendor ", "verify ", "wallet ", "wholesale ", "balance due ", "invoice paid ",
    "payment link ", "payment method ", "payment received ", "refund issued ", "sale complete ", "send invoice ", "send payment ", "",
];

// DICT_E: general phrase pack for casual, everyday, and high-frequency conversational chunks.
#[rustfmt::skip]
pub const DICT_E: [&str; 256] = [
    " ", "!", ",", ".", "...", "?", "😎", "🙏",
    "🔥", "😂", "❤️", "🚀", "👀", "💀", "💯", "🤣",
    "🤯", "🫡", "a ", "about ", "after ", "again ", "all ", "already ",
    "am ", "and ", "any ", "are ", "as ", "at ", "be ", "because ",
    "before ", "but ", "by ", "can ", "could ", "did ", "do ", "does ",
    "done ", "for ", "from ", "get ", "go ", "going ", "good ", "got ",
    "had ", "has ", "have ", "help ", "here ", "how ", "i ", "if ",
    "in ", "is ", "it ", "just ", "know ", "later ", "let ", "like ",
    "look ", "make ", "maybe ", "me ", "more ", "my ", "need ", "new ",
    "next ", "no ", "not ", "now ", "of ", "ok ", "on ", "or ",
    "please ", "really ", "right ", "see ", "send ", "should ", "soon ", "still ",
    "sure ", "take ", "tell ", "than ", "thank ", "thanks ", "that ", "the ",
    "then ", "there ", "they ", "this ", "time ", "to ", "today ", "tomorrow ",
    "too ", "try ", "up ", "us ", "want ", "was ", "way ", "we ",
    "well ", "were ", "what ", "when ", "where ", "who ", "why ", "will ",
    "with ", "work ", "would ", "yes ", "you ", "your ", "exactly ", "shipment delayed ",
    "how so ", "all systems healthy ", "same here ", "for example ", "perhaps ", "call me ",
    "probably ", "mostly ", "can you check ", "especially ", "can you send ", "eventually ",
    "testing underway ", "sometimes ", "quite a bit ", "interesting observation ", "sounds reasonable ", "one moment ",
    "are you ready ", "for sure ", "test results are in ", "asset metadata updated ", "good idea ", "good morning ",
    "good night ", "got it ", "great job ", "system is down ", "all systems go ", "how are you ",
    "how is everything ", "i agree ", "i am here ", "i am not sure ", "i am ready ", "i am working on it ",
    "system is up ", "i can do that ", "i can help ", "under investigation ", "all tests passed ", "i dont know ",
    "where is my order ", "tracking number ", "i hope so ", "i just saw this ", "i like that ", "i need help ",
    "i need to check ", "i saw it ", "i sent it ", "i should be there ", "i think so ", "i understand ",
    "i want to know ", "live stream up ", "i will call ", "i will check ", "i will let you know ", "i will send it ",
    "i will try ", "i would like ", "if you can ", "ill check ", "is it ready ", "it is done ",
    "it is fine ", "it looks good ", "it should work ", "its all good ", "its ok ", "just checking ",
    "just let me know ", "new nft set ", "let me check ", "let me know ", "let me see ", "looks good ",
    "maybe later ", "need help ", "no problem ", "no worries ", "not sure ", "on my way ",
    "one minute ", "please call ", "please check ", "please help ", "please let me know ", "please send ",
    "right now ", "released ", "send it over ", "send me ", "sounds good ", "sounds like ",
    "talk soon ", "talk to you later ", "thank you ", "thanks again ", "that is good ", "that makes sense ",
    "that should work ", "thats crazy ", "thats good ", "this is good ", "this looks good ", "try again ",
    "want me to ", "we can do that ", "we got this ", "we need to check ", "we should check ", "what are you doing ",
    "what do you mean ", "what do you think ", "why is this not working ", "who needs help ", "what time ", "when you can ",
    "where are you ", "will do ", "you are right ", "you got it ", "you know ", "you should check ",
    "you want to ", "",
];

// DICT_F: asset-owner, holder, announcement, notice, document, IPFS, NFT, collection, and update language.
#[rustfmt::skip]
pub const DICT_F: [&str; 256] = [
    " ", "!", ",", ".", "=", "?", "$", "%",
    "😎", "🙏", "🔥", "🚀", "'s ", "able ", "al ", "ation ",
    "d ", "e ", "ed ", "er ", "ers ", "es ", "est ", "ful ",
    "ies ", "ing ", "ion ", "ions ", "less ", "ly ", "ment ", "ness ",
    "s ", "tion ", "tions ", "y ", "a ", "about ", "after ", "all ",
    "already ", "and ", "any ", "are ", "as ", "at ", "be ", "because ",
    "before ", "between ", "but ", "by ", "can ", "do ", "done ", "for ",
    "from ", "have ", "here ", "if ", "in ", "into ", "is ", "it ",
    "just ", "know ", "later ", "me ", "my ", "need ", "new ", "next ",
    "no ", "not ", "now ", "of ", "ok ", "on ", "or ", "please ",
    "ready ", "send ", "should ", "soon ", "still ", "than ", "thank ", "thank you ",
    "thanks ", "that ", "the ", "their ", "then ", "there ", "they ", "this ",
    "through ", "time ", "to ", "today ", "tomorrow ", "until ", "us ", "use ",
    "we ", "when ", "where ", "while ", "will ", "with ", "without ", "yes ",
    "you ", "your ", "account ", "access ", "action ", "active ", "add ", "alert ",
    "announce ", "announcement ", "archive ", "artwork ", "asset ", "attach ", "attachment ", "avail ",
    "backup ", "chang ", "check ", "claim ", "clos ", "collection ", "comment ", "community ",
    "confirm ", "content ", "current ", "detail ", "document ", "drop ", "download ", "draft ",
    "edition ", "evidence ", "file ", "final ", "follow ", "holder ", "image ", "important ",
    "info ", "inform ", "ipfs ", "issue ", "latest ", "link ", "lock ", "media ",
    "member ", "message ", "metadata ", "mint ", "notice ", "nft ", "official ", "open ",
    "owner ", "post ", "previous ", "private ", "proof ", "public ", "publish ", "read ",
    "record ", "referenc ", "release ", "reveal ", "reminder ", "remov ", "replac ", "report ",
    "request ", "requir ", "review ", "save ", "share ", "show ", "sign ", "statement ",
    "status ", "storage ", "submit ", "support ", "updat ", "upload ", "user ", "valid ",
    "verifi ", "version ", "view ", "warning ", "action required ", "all holders ", "asset holder ", "asset update ",
    "available now ", "check latest ", "check the file ", "claim available ", "details available ", "document available ", "file available ", "holder notice ",
    "holder update ", "important notice ", "latest document ", "latest file ", "latest update ", "new attachment ", "new document ", "new file ",
    "no action required ", "official update ", "owner message ", "please confirm ", "please read ", "please review ", "proof attached ", "public notice ",
    "record updated ", "review before ", "status changed ", "update available ", "version updated ", "ipfs file ", "ipfs update ", "message posted ",
    "notice posted ", "member update ", "community notice ", "stay tuned ", "big update ", "let me know ", "we are live ", "more info soon ",
    "owner update ", "read more ", "update posted ", "drop live ", "holder only ", "mint open ", "new collection ", "",
];

// DICT_G: traceability, logistics, provenance, movement, custody, and product state.
#[rustfmt::skip]
pub const DICT_G: [&str; 256] = [
    " ", "!", ",", ".", "=", "?", "$", "%",
    "😎", "🙏", "'s ", "able ", "al ", "ation ", "d ", "e ",
    "ed ", "er ", "es ", "ies ", "ing ", "ion ", "ions ", "ly ",
    "ment ", "s ", "tion ", "a ", "about ", "after ", "all ", "already ",
    "am ", "and ", "any ", "are ", "as ", "at ", "be ", "because ",
    "before ", "between ", "but ", "by ", "can ", "could ", "did ", "do ",
    "does ", "done ", "during ", "for ", "from ", "get ", "give ", "had ",
    "has ", "have ", "help ", "here ", "how ", "i ", "if ", "in ",
    "into ", "is ", "it ", "just ", "know ", "later ", "let me know ", "me ",
    "my ", "need ", "new ", "next ", "no ", "not ", "now ", "of ",
    "ok ", "on ", "or ", "please ", "ready ", "send ", "sent ", "should ",
    "since ", "soon ", "still ", "sure ", "than ", "thank ", "thank you ", "thanks ",
    "that ", "the ", "their ", "them ", "then ", "there ", "they ", "this ",
    "through ", "time ", "to ", "today ", "tomorrow ", "until ", "us ", "use ",
    "want ", "we ", "week ", "when ", "where ", "which ", "while ", "will ",
    "with ", "without ", "yes ", "you ", "your ", "accept ", "address ", "allocat ",
    "arriv ", "asset ", "attach ", "avail ", "back ", "backup ", "batch ", "box ",
    "bundle ", "carrier ", "carton ", "chain ", "check ", "client ", "code ", "collect ",
    "complete ", "confirm ", "container ", "count ", "courier ", "creat ", "customer ", "date ",
    "day ", "delay ", "depart ", "deliver ", "delivery ", "destination ", "detail ", "dispatch ",
    "distribut ", "document ", "driver ", "drop ", "entry ", "expect ", "export ", "facility ",
    "file ", "final ", "finish ", "fix ", "freight ", "fulfill ", "handle ", "handoff ",
    "hold ", "hub ", "id ", "import ", "inspect ", "inventory ", "item ", "label ",
    "load ", "local ", "locat ", "log ", "lot ", "manifest ", "manufactur ", "material ",
    "move ", "note ", "number ", "origin ", "outbound ", "pack ", "package ", "pallet ",
    "parcel ", "part ", "pickup ", "place ", "point ", "process ", "produc ", "product ",
    "provid ", "quality ", "quantity ", "receiv ", "record ", "referenc ", "release ", "report ",
    "reserve ", "retail ", "return ", "review ", "route ", "scan ", "seal ", "serial ",
    "ship ", "shipment ", "shop ", "source ", "stage ", "status ", "stock ", "stor ",
    "store ", "supplier ", "supply ", "tag ", "track ", "trace ", "transfer ", "transit ",
    "transport ", "unit ", "updat ", "verify ", "warehouse ", "weight ", "work ", "arrived at ",
    "available for pickup ", "in route ", "in transit ", "ready for pickup ", "received at ", "released to ", "source verified ",
    "",
];

// DICT_H: crypto, Hemp0x, on-chain, wallet, mining, swap, meme, IPFS, and tech language.
#[rustfmt::skip]
pub const DICT_H: [&str; 256] = [
    " ", "!", ",", ".", "=", "?", "$", "%",
    "😎", "🙏", "'s ", "able ", "al ", "ation ", "d ", "e ",
    "ed ", "er ", "ers ", "es ", "est ", "ful ", "ies ", "ing ",
    "ion ", "ions ", "less ", "ly ", "ment ", "ness ", "s ", "tion ",
    "tions ", "y ", "a ", "about ", "account ", "address ", "after ", "again ",
    "airdrop ", "all ", "already ", "am ", "amount ", "and ", "any ", "api ",
    "app ", "are ", "as ", "asset ", "asset holder ", "at ", "attach ", "backup ",
    "balance ", "be ", "because ", "before ", "between ", "block ", "social ", "burn ",
    "but ", "buy ", "by ", "can ", "chain ", "chart ", "claim ", "cli ",
    "client ", "code ", "command ", "community ", "confirm ", "connect ", "core ", "could ",
    "data ", "commander ", "deploy ", "dev ", "develop ", "dex ", "did ", "difficulty ",
    "do ", "document ", "does ", "domain ", "donat ", "done ", "download ", "during ",
    "endpoint ", "error ", "explorer ", "fee ", "file ", "final ", "fix ", "for ",
    "from ", "fund ", "get ", "give ", "had ", "has ", "hash ", "hash verified ",
    "hashrate ", "have ", "height ", "help ", "hemp ", "hemp0x ", "here ", "hold ",
    "holder ", "holder update ", "how ", "i ", "if ", "in ", "index ", "input ",
    "into ", "ipfs ", "is ", "issue ", "it ", "just ", "key ", "know ",
    "later ", "ledger ", "let me know ", "link ", "liquid ", "lock ", "log ", "mainnet ",
    "me ", "meme ", "message ", "metadata ", "min ", "mint ", "my ", "need ",
    "network ", "new ", "next ", "no ", "node ", "nonce ", "not ", "now ",
    "of ", "ok ", "on ", "or ", "output ", "owner ", "pair ", "peer ",
    "please ", "pool ", "pow ", "price ", "proof ", "protocol ", "provid ", "ready ",
    "reward ", "rpc ", "webcom ", "seed ", "sell ", "send ", "sent ", "server ",
    "setting ", "share ", "should ", "sign ", "discord ", "since ", "soon ", "source ",
    "stake ", "state ", "status ", "still ", "storage ", "sure ", "swap ", "sync ",
    "test ", "testnet ", "than ", "thank ", "thank you ", "thanks ", "that ", "the ",
    "their ", "them ", "then ", "there ", "they ", "this ", "through ", "time ",
    "to ", "today ", "token ", "tomorrow ", "transaction ", "transaction confirmed ", "transfer ", "tx ",
    "txid ", "unlock ", "until ", "updat ", "upgrade ", "us ", "use ", "valid ",
    "verify ", "version ", "wallet ", "wallet address ", "want ", "we ", "web ", "week ",
    "when ", "where ", "which ", "while ", "why ", "will ", "with ", "withdraw ",
    "without ", "work ", "system ", "would ", "yes ", "you ", "your ", "",
];

#[rustfmt::skip]
pub const DICTIONARIES: [(&str, &[&str]); 8] = [
    ("A", &DICT_A),
    ("B", &DICT_B),
    ("C", &DICT_C),
    ("D", &DICT_D),
    ("E", &DICT_E),
    ("F", &DICT_F),
    ("G", &DICT_G),
    ("H", &DICT_H),
];

#[rustfmt::skip]
pub const ACRONYMS: &[&str] = &[
    "ach", "ai", "aml", "api", "apr", "apy", "asic", "ath", "atl", "av", "btc", "btw", "cid",
    "cli", "cpu", "css", "db", "defi", "dev", "dht", "dns", "fdv", "fr", "gb", "gm", "gn",
    "gpu", "gui", "hemp", "html", "http", "https", "id", "idk", "imo", "ip", "ipfs", "irl",
    "json", "kyc", "llm", "lol", "lmao", "mb", "msrp", "nft", "ngl", "otc", "omg", "p2p",
    "pdf", "pin", "po", "pos", "pow", "pr", "qr", "ram", "roi", "rpc", "sdk", "seo", "sku",
    "sms", "sql", "ssl", "svg", "tbh", "tls", "tor", "tx", "txid", "ui", "url", "usb", "utxo",
    "ux", "vpn", "wasm", "ws", "wip", "wtf", "xml", "i",
];

// `table_identity` produces a stable, deterministic fingerprint of every
// dictionary and suffix currently compiled in. It is intentionally sensitive to
// reorders, deletions, additions, and phrase edits, so a receiver can tell which
// table pack was used before decoding. The byte stream hashed is:
//
//   HOXSHT_VERSION_MARKER (utf-8)
//   for each (name, dict) in DICTIONARIES in order:
//     name (utf-8) + 0x00 + len(dict) as u32 little-endian
//     for each phrase in dict:
//       len(phrase) as u32 little-endian + phrase (utf-8) + 0x1f
//   for each suffix in SUFFIXES:
//     len(suffix) as u32 little-endian + suffix (utf-8) + 0x1f
//
// This is the format the audit binary will print in the `Table Identity`
// section of the report. It encodes length prefixes so adjacent strings cannot
// collide when entries are inserted, removed, or edited.
pub fn table_identity() -> TableIdentity {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(HOXSHT_VERSION_MARKER.as_bytes());
    hasher.update([0u8]);

    let mut entry_counts: Vec<(String, usize)> = Vec::with_capacity(DICTIONARIES.len());
    for (name, dict) in DICTIONARIES.iter() {
        hasher.update(name.as_bytes());
        hasher.update([0u8]);
        let count = dict.len() as u32;
        hasher.update(count.to_le_bytes());
        for phrase in dict.iter() {
            let len = phrase.len() as u32;
            hasher.update(len.to_le_bytes());
            hasher.update(phrase.as_bytes());
            hasher.update([0x1fu8]);
        }
        entry_counts.push((name.to_string(), dict.len()));
    }

    for suffix in SUFFIXES.iter() {
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

    TableIdentity {
        version_marker: HOXSHT_VERSION_MARKER.to_string(),
        dictionary_count: DICTIONARIES.len(),
        entry_counts,
        suffix_count: SUFFIXES.len(),
        fingerprint_sha256: hex,
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TableIdentity {
    pub version_marker: String,
    pub dictionary_count: usize,
    pub entry_counts: Vec<(String, usize)>,
    pub suffix_count: usize,
    pub fingerprint_sha256: String,
}
