import { writable, derived } from "svelte/store";

export const contentLibrary = writable([]);
export const libraryLoading = writable(false);
export const activePanel = writable("browse");
export const searchQuery = writable("");
export const statusFilter = writable("all");
export const ipfsHubSection = writable("library");

export function sortByUpdatedDesc(packages) {
    return [...packages].sort((a, b) => b.updated_at.localeCompare(a.updated_at));
}

export const filteredPackages = derived(
    [contentLibrary, searchQuery, statusFilter],
    ([$contentLibrary, $searchQuery, $statusFilter]) => {
        let pkgs = sortByUpdatedDesc($contentLibrary);
        if ($searchQuery.trim()) {
            const q = $searchQuery.toLowerCase();
            pkgs = pkgs.filter((p) =>
                p.name.toLowerCase().includes(q) ||
                (p.description && p.description.toLowerCase().includes(q)) ||
                (p.tags && p.tags.some((t) => t.toLowerCase().includes(q))) ||
                (p.cid && p.cid.toLowerCase().includes(q))
            );
        }
        if ($statusFilter === "local") {
            pkgs = pkgs.filter((p) => p.status === "local" || !p.status);
        } else if ($statusFilter === "external") {
            pkgs = pkgs.filter((p) => p.status === "external");
        } else if ($statusFilter === "published") {
            pkgs = pkgs.filter((p) => p.status === "published");
        }
        return pkgs;
    }
);
