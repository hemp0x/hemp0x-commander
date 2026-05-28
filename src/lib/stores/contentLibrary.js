import { writable } from "svelte/store";

export const contentLibrary = writable([]);
export const libraryLoading = writable(false);
export const activePanel = writable("browse");

export function sortByUpdatedDesc(packages) {
    return [...packages].sort((a, b) => b.updated_at.localeCompare(a.updated_at));
}
