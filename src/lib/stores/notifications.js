import { writable, derived } from "svelte/store";

const STORAGE_KEY = "hemp0x_notifications";
const MAX_ITEMS = 100;
const DEDUPE_WINDOW_MS = 5000;

function getStorage() {
    if (typeof localStorage === "undefined") return null;
    return localStorage;
}

function normalizeNotification(item) {
    if (!item || typeof item !== "object") return null;
    return {
        id: typeof item.id === "string" ? item.id : generateId(),
        type: typeof item.type === "string" ? item.type : "system",
        severity:
            typeof item.severity === "string" ? item.severity : "info",
        title: typeof item.title === "string" ? item.title : "",
        body: typeof item.body === "string" ? item.body : "",
        timestamp:
            typeof item.timestamp === "number" ? item.timestamp : Date.now(),
        read: Boolean(item.read),
        action: item.action && typeof item.action === "object" ? item.action : null,
        persist: item.persist !== false,
    };
}

function loadPersisted() {
    try {
        const storage = getStorage();
        if (!storage) return [];
        const raw = storage.getItem(STORAGE_KEY);
        if (raw) {
            const items = JSON.parse(raw);
            if (Array.isArray(items)) {
                return items
                    .map(normalizeNotification)
                    .filter(Boolean)
                    .slice(0, MAX_ITEMS);
            }
        }
    } catch {
        // corrupted data, start fresh
    }
    return [];
}

function persist(items) {
    try {
        const storage = getStorage();
        if (!storage) return;
        const persistedItems = items.filter((item) => item.persist !== false);
        storage.setItem(
            STORAGE_KEY,
            JSON.stringify(persistedItems.slice(0, MAX_ITEMS)),
        );
    } catch {
        // storage full or unavailable
    }
}

let nextId = 0;

function generateId() {
    return `nc_${Date.now()}_${++nextId}`;
}

function severityFromToastType(type) {
    if (type === "error") return "error";
    if (type === "success") return "success";
    if (type === "warning") return "warning";
    return "info";
}

function createNotificationStore() {
    const initial = loadPersisted();
    const { subscribe, set, update } = writable(initial);

    function add(notification) {
        update((items) => {
            const now = Date.now();
            const dedupeKey = [
                notification.type || "system",
                notification.severity || "info",
                notification.title || "",
                notification.body || "",
            ].join("|");

            const recentDuplicate = items.find(
                (n) =>
                    [
                        n.type || "system",
                        n.severity || "info",
                        n.title || "",
                        n.body || "",
                    ].join("|") === dedupeKey &&
                    now - n.timestamp < DEDUPE_WINDOW_MS,
            );
            if (recentDuplicate) return items;

            const entry = {
                id: generateId(),
                type: notification.type || "system",
                severity: notification.severity || "info",
                title: notification.title || "",
                body: notification.body || "",
                timestamp: now,
                read: false,
                action: notification.action || null,
                persist:
                    notification.persist ?? notification.severity !== "error",
            };

            const next = [entry, ...items].slice(0, MAX_ITEMS);
            persist(next);
            return next;
        });
    }

    function markRead(id) {
        update((items) => {
            const next = items.map((n) => (n.id === id ? { ...n, read: true } : n));
            persist(next);
            return next;
        });
    }

    function markAllRead() {
        update((items) => {
            const next = items.map((n) => ({ ...n, read: true }));
            persist(next);
            return next;
        });
    }

    function clear(id) {
        update((items) => {
            const next = items.filter((n) => n.id !== id);
            persist(next);
            return next;
        });
    }

    function clearAll() {
        update(() => {
            persist([]);
            return [];
        });
    }

    function addToast(msg, type) {
        add({
            type: "system",
            severity: severityFromToastType(type),
            title: "",
            body: msg,
        });
    }

    return {
        subscribe,
        set,
        update,
        add,
        addToast,
        markRead,
        markAllRead,
        clear,
        clearAll,
    };
}

export const notifications = createNotificationStore();

export function addNotification(notification) {
    notifications.add(notification);
}

export function addToastNotification(msg, type) {
    notifications.addToast(msg, type);
}

function compactBody(body, maxLength = 200) {
    const text = String(body || "");
    if (text.length <= maxLength) return text;
    return `${text.slice(0, maxLength - 3)}...`;
}

export function addRuntimeNotification(title, body, severity = "info", action = null) {
    notifications.add({
        type: "runtime",
        severity,
        title,
        body: compactBody(body),
        action,
        persist: severity !== "error",
    });
}

export function addTransactionNotification(title, body, severity, txid) {
    notifications.add({
        type: "transaction",
        severity,
        title,
        body: compactBody(body),
        action: txid ? { label: "Copy TXID", txid } : null,
    });
}

export function addToolNotification(title, body, severity = "info", action = null, persist = undefined) {
    notifications.add({
        type: "tool",
        severity,
        title,
        body: compactBody(body),
        action,
        persist: persist ?? severity !== "error",
    });
}

export const unreadCount = derived(notifications, ($n) =>
    $n.filter((n) => !n.read).length,
);
