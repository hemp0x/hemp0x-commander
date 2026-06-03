/**
 * Format a balance for display.
 * - < 10,000: Full precision (8 decimals), strip trailing zeros.
 * - 10k - 1M: 1 decimal place with 'K' suffix.
 * - 1M - 1B: 2 decimal places with 'M' suffix.
 * - > 1B: 2 decimal places with 'B' suffix.
 * @param {string|number} balance
 * @returns {string}
 */
export function formatBalance(balance) {
    const num = parseFloat(balance);
    if (isNaN(num)) return String(balance);

    const absNum = Math.abs(num);

    if (absNum >= 1_000_000_000) {
        return (num / 1_000_000_000).toFixed(2).replace(/\.?0+$/, "") + "B";
    } else if (absNum >= 1_000_000) {
        return (num / 1_000_000).toFixed(2).replace(/\.?0+$/, "") + "M";
    } else if (absNum >= 10_000) {
        return (num / 1_000).toFixed(1).replace(/\.?0+$/, "") + "K";
    } else {
        return num.toFixed(8).replace(/\.?0+$/, "");
    }
}

/**
 * Format a raw amount with commas for standard display (no suffix)
 * @param {string|number} amount 
 * @returns {string}
 */
export function formatAmount(amount) {
    const num = parseFloat(amount);
    if (isNaN(num)) return "0.00";
    return num.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 8 });
}

/**
 * Smart suggestion insertion for short-message autocomplete.
 * - Appends after whitespace with natural casing (lowercased first word mid-sentence).
 * - Replaces trailing partial token when no trailing whitespace.
 * - Preserves acronyms (IPFS, CID, HEMP, API, RPC, UTXO).
 * @param {string} currentText
 * @param {string} suggestion
 * @returns {string}
 */
export function insertSuggestion(currentText, suggestion) {
    const cleanSuggestion = suggestion?.trimStart() ?? "";
    if (!cleanSuggestion) return currentText ?? "";
    if (!currentText || !currentText.trim()) return cleanSuggestion;

    const trimmedEnd = currentText.trimEnd();
    const endsInWhitespace = currentText.length > trimmedEnd.length;

    if (endsInWhitespace) {
        const suffix = stripOverlappingPrefix(trimmedEnd, cleanSuggestion);
        return currentText + normalizeSuggestionForContext(trimmedEnd, suffix);
    }

    // Replace trailing partial token with the full suggestion
    const match = trimmedEnd.match(/(\S+)$/);
    if (!match) {
        return currentText + " " + normalizeSuggestionForContext(trimmedEnd, cleanSuggestion);
    }

    const partialToken = match[1];
    const beforePartial = currentText.slice(0, currentText.length - partialToken.length);
    const context = beforePartial.trimEnd();
    const suffix = stripOverlappingPrefix(context, cleanSuggestion);
    return beforePartial + normalizeSuggestionForContext(context, suffix);
}

function normalizeSuggestionForContext(contextText, suggestion) {
    if (!suggestion) {
        return "";
    }
    const isStartOfSentence =
        contextText.length === 0 || /[.!?]\s*$/.test(contextText);
    if (isStartOfSentence) {
        return suggestion;
    }
    return lowerCaseFirstUnlessAcronym(suggestion);
}

function lowerCaseFirstUnlessAcronym(suggestion) {
    const ACRONYMS = new Set([
        "api",
        "cid",
        "gm",
        "gn",
        "hemp",
        "idk",
        "imo",
        "ipfs",
        "irl",
        "json",
        "lol",
        "lmao",
        "nft",
        "ngl",
        "omg",
        "p2p",
        "pr",
        "rpc",
        "sdk",
        "sql",
        "tbh",
        "tx",
        "txid",
        "ui",
        "url",
        "utxo",
        "ux",
        "wtf",
    ]);
    const words = suggestion.split(/(\s+)/);
    const firstWord = words[0];
    const firstLower = firstWord.toLowerCase();

    if (ACRONYMS.has(firstLower)) {
        return suggestion;
    }

    words[0] = firstWord.charAt(0).toLowerCase() + firstWord.slice(1);
    return words.join("");
}

function stripOverlappingPrefix(contextText, suggestion) {
    const contextWords = tokenizeWords(contextText);
    const suggestionWords = tokenizeWords(suggestion);
    const overlap = sharedSuffixPrefixLength(contextWords, suggestionWords);

    if (overlap === 0) {
        return suggestion;
    }

    const keptWords = suggestionWords.slice(overlap);
    if (keptWords.length === 0) {
        return "";
    }

    const leadingWhitespace = suggestion.match(/^\s*/)?.[0] ?? "";
    const trailingWhitespace = suggestion.match(/\s*$/)?.[0] ?? "";
    return `${leadingWhitespace}${keptWords.join(" ")}${trailingWhitespace}`;
}

function sharedSuffixPrefixLength(contextWords, suggestionWords) {
    const maxOverlap = Math.min(contextWords.length, suggestionWords.length);
    for (let count = maxOverlap; count > 0; count -= 1) {
        const contextTail = contextWords
            .slice(contextWords.length - count)
            .map((word) => word.toLowerCase());
        const suggestionHead = suggestionWords
            .slice(0, count)
            .map((word) => word.toLowerCase());
        if (contextTail.join("\u0000") === suggestionHead.join("\u0000")) {
            return count;
        }
    }
    return 0;
}

function tokenizeWords(text) {
    return text
        .trim()
        .split(/\s+/)
        .filter(Boolean);
}
