// Shared helpers for the local Runtime Wallet PIN unlock layer (slice 76b).
//
// The PIN is a device-local convenience for unlocking the active Core runtime
// wallet for signing/broadcast. It does NOT protect the portable Hemp0x Vault
// file. These helpers wrap the backend `wallet_pin_*` Tauri commands so every
// wallet-unlock prompt can offer a consistent PIN/passphrase UX without
// duplicating logic.

import { core } from "@tauri-apps/api";

export const WALLET_PIN_LENGTH = 6;

/**
 * Fetch the current wallet-PIN status from the backend.
 * @returns {Promise<{pin_configured: boolean, requires_passphrase: boolean, lockout_remaining_secs: number, reason: string} | null>}
 */
export async function loadWalletPinStatus() {
    try {
        return await core.invoke("wallet_pin_status");
    } catch {
        return null;
    }
}

/**
 * Decide the default unlock mode given a freshly fetched status object.
 * @returns {"pin" | "passphrase"}
 */
export function defaultUnlockMode(status) {
    if (status && status.pin_configured && !status.requires_passphrase) {
        return "pin";
    }
    return "passphrase";
}

/**
 * Whether the PIN layer currently requires the full wallet passphrase even
 * though a PIN is configured (rotation / wallet changed / lockout / identity
 * unavailable).
 */
export function pinRequiresPassphrase(status) {
    return !!(status && status.pin_configured && status.requires_passphrase);
}

/**
 * Unlock the active runtime wallet with the device PIN.
 * @param {string} pin
 * @param {number} [duration] seconds (default 300)
 */
export async function unlockWalletWithPin(pin, duration = 300) {
    return core.invoke("wallet_pin_unlock", { pin, duration });
}

/**
 * "Forgot PIN?" clears the local wallet-PIN record without the passphrase.
 */
export async function forgotWalletPin() {
    return core.invoke("wallet_pin_forgot");
}

/**
 * Unlock the **active runtime wallet** with its full wallet passphrase.
 *
 * This is the single correct entry point for normal runtime wallet unlock
 * passphrase fallbacks: it routes to the named active vault wallet when one is
 * set, and to the default wallet otherwise, exactly matching the PIN unlock
 * routing. Use this instead of the legacy `wallet_unlock` command from any
 * send / asset / chat / consolidation / global wallet unlock prompt so the
 * passphrase fallback and the PIN fallback unlock the same active wallet.
 *
 * @param {string} password
 * @param {number} [duration] seconds (default 300)
 */
export async function unlockRuntimeWalletWithPassphrase(password, duration = 300) {
    return core.invoke("wallet_unlock_active", { password, duration });
}

/**
 * Set up a device PIN for the active runtime wallet.
 * @param {string} walletPassphrase
 * @param {string} pin
 * @param {number} [duration] seconds the wallet stays unlocked after setup (default 300)
 */
export async function setupWalletPin(walletPassphrase, pin, duration = 300) {
    return core.invoke("wallet_pin_setup", { walletPassphrase, pin, duration });
}

/**
 * Change the device PIN.
 */
export async function changeWalletPin(walletPassphrase, newPin) {
    return core.invoke("wallet_pin_change", { walletPassphrase, newPin });
}

/**
 * Remove the device PIN.
 */
export async function removeWalletPin(walletPassphrase) {
    return core.invoke("wallet_pin_remove", { walletPassphrase });
}

/**
 * Validate a PIN string for UI gating: exactly 6 ASCII digits.
 */
export function isValidPin(pin) {
    return typeof pin === "string" && /^\d{6}$/.test(pin);
}

/**
 * Normalize user-entered PIN text for PIN inputs.
 */
export function cleanWalletPin(value) {
    return String(value || "").replace(/\D/g, "").slice(0, WALLET_PIN_LENGTH);
}
