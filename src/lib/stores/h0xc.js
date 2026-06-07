export function deriveRootNameFn(assetName) {
    if (!assetName) return "";
    const clean = String(assetName).replace(/!$/, "");
    const upper = clean.toUpperCase();
    if (upper.endsWith("/H0XC")) return clean.split("/")[0] || "";
    if (upper.endsWith(".H0XC")) return clean.slice(0, -5);
    if (upper === "H0XC") return "";
    return clean;
}

export function isH0xCAsset(name) {
    if (!name) return false;
    const upper = String(name).toUpperCase();
    return upper.endsWith("/H0XC")
        || upper.endsWith(".H0XC")
        || upper.endsWith("/H0XC!")
        || upper.endsWith(".H0XC!");
}

export function isH0xCChannelAsset(name) {
    if (!name) return false;
    const upper = String(name).toUpperCase();
    return upper.endsWith("/H0XC") || upper.endsWith(".H0XC");
}
