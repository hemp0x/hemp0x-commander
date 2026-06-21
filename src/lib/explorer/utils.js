const TXID_PATTERN = /^[0-9a-fA-F]{64}$/;
const ADDRESS_PATTERN = /^R[1-9A-HJ-NP-Za-km-z]{25,34}$/;

export function classifyExplorerTarget(value) {
    const target = String(value || "").trim();
    if (TXID_PATTERN.test(target)) return "transaction";
    if (ADDRESS_PATTERN.test(target)) return "address";
    return null;
}

export function formatAmount(value, digits = 8) {
    if (value === null || value === undefined || value === "") return "--";
    const number = Number(value);
    if (!Number.isFinite(number)) return String(value);
    return number.toLocaleString(undefined, {
        minimumFractionDigits: 0,
        maximumFractionDigits: digits,
    });
}

export function formatInteger(value) {
    const number = Number(value);
    if (!Number.isFinite(number)) return value ?? "--";
    return Math.trunc(number).toLocaleString();
}

export function formatDate(value) {
    if (value === null || value === undefined || value === "") return "--";
    const raw = Number(value);
    const date = Number.isFinite(raw)
        ? new Date(raw < 10_000_000_000 ? raw * 1000 : raw)
        : new Date(value);
    if (Number.isNaN(date.getTime())) return String(value);
    return date.toLocaleString();
}

export function firstDefined(source, keys, fallback = null) {
    for (const key of keys) {
        const value = source?.[key];
        if (value !== undefined && value !== null) return value;
    }
    return fallback;
}

export function readAddress(source) {
    const direct = firstDefined(source, [
        "address",
        "from_address",
        "to_address",
    ]);
    if (direct) return String(direct);

    const addresses =
        source?.addresses ||
        source?.scriptPubKey?.addresses ||
        source?.script_pub_key?.addresses;
    if (Array.isArray(addresses) && addresses.length) {
        return String(addresses[0]);
    }

    return "";
}

export function isUnsupportedIndexError(error) {
    if (error && typeof error === "object") {
        if (
            error.unsupported_index === true ||
            error.code === "UNSUPPORTED_INDEX" ||
            error.kind === "unsupported_index"
        ) {
            return true;
        }
    }

    const message = String(error?.message || error || "").toLowerCase();
    return [
        "unsupported index",
        "index is disabled",
        "index not enabled",
        "txindex",
        "addressindex",
        "address index",
        "reindex",
    ].some((phrase) => message.includes(phrase));
}

export function hasUnsupportedIndexFlag(result) {
    return Boolean(
        result?.unsupported_index ||
        result?.unsupportedIndex ||
        result?.index_available === false ||
        result?.indexAvailable === false ||
        result?.supported === false,
    );
}
