import { writable, derived } from "svelte/store";

export const contentLibrary = writable([]);
export const libraryLoading = writable(false);
export const activePanel = writable("browse");
export const searchQuery = writable("");
export const statusFilter = writable("all");
export const currentFolder = writable(null); // null = root, '' = unsorted, 'name' = folder
export const ipfsHubSection = writable("library");
export const packageSortMode = writable("updated-desc"); // alpha-asc, alpha-desc, updated-newest, updated-oldest

export function sortByUpdatedDesc(packages) {
    return [...packages].sort((a, b) => b.updated_at.localeCompare(a.updated_at));
}

export function getFolders(packages) {
    const folders = new Set();
    for (const p of packages) {
        if (p.folder && p.folder.trim()) {
            folders.add(p.folder.trim());
        }
    }
    return Array.from(folders).sort((a, b) => a.localeCompare(b));
}

export function groupByFolder(packages) {
    const groups = new Map();
    const unsorted = [];
    for (const p of packages) {
        const folder = p.folder && p.folder.trim() ? p.folder.trim() : "";
        if (!folder) {
            unsorted.push(p);
        } else {
            if (!groups.has(folder)) groups.set(folder, []);
            groups.get(folder).push(p);
        }
    }
    const result = [];
    if (unsorted.length > 0) {
        result.push({ folder: "", label: "Unsorted", packages: sortByUpdatedDesc(unsorted) });
    }
    const sortedFolders = Array.from(groups.keys()).sort((a, b) => a.localeCompare(b));
    for (const folder of sortedFolders) {
        result.push({ folder, label: folder, packages: sortByUpdatedDesc(groups.get(folder)) });
    }
    return result;
}

function sortPackages(pkgs, mode) {
    const list = [...pkgs];
    if (mode === "alpha-asc") {
        return list.sort((a, b) => a.name.localeCompare(b.name));
    } else if (mode === "alpha-desc") {
        return list.sort((a, b) => b.name.localeCompare(a.name));
    } else if (mode === "updated-oldest") {
        return list.sort((a, b) => a.updated_at.localeCompare(b.updated_at));
    }
    // updated-newest / default
    return list.sort((a, b) => b.updated_at.localeCompare(a.updated_at));
}

export const filteredPackages = derived(
    [contentLibrary, searchQuery, statusFilter, currentFolder, packageSortMode],
    ([$contentLibrary, $searchQuery, $statusFilter, $currentFolder, $packageSortMode]) => {
        let pkgs = sortPackages($contentLibrary, $packageSortMode);
        if ($searchQuery.trim()) {
            const q = $searchQuery.toLowerCase();
            pkgs = pkgs.filter((p) =>
                p.name.toLowerCase().includes(q) ||
                (p.description && p.description.toLowerCase().includes(q)) ||
                (p.tags && p.tags.some((t) => t.toLowerCase().includes(q))) ||
                (p.cid && p.cid.toLowerCase().includes(q)) ||
                (p.folder && p.folder.toLowerCase().includes(q))
            );
        }
        if ($statusFilter === "local") {
            pkgs = pkgs.filter((p) => p.status === "local" || !p.status);
        } else if ($statusFilter === "external") {
            pkgs = pkgs.filter((p) => p.status === "external");
        } else if ($statusFilter === "published") {
            pkgs = pkgs.filter((p) => p.status === "published");
        }
        if ($currentFolder !== null) {
            if ($currentFolder === "") {
                pkgs = pkgs.filter((p) => !p.folder || !p.folder.trim());
            } else {
                pkgs = pkgs.filter((p) => p.folder && p.folder.trim() === $currentFolder);
            }
        }
        return pkgs;
    }
);

export const folderGroups = derived(
    [contentLibrary, searchQuery, statusFilter, packageSortMode],
    ([$contentLibrary, $searchQuery, $statusFilter, $packageSortMode]) => {
        let pkgs = sortPackages($contentLibrary, $packageSortMode);
        if ($searchQuery.trim()) {
            const q = $searchQuery.toLowerCase();
            pkgs = pkgs.filter((p) =>
                p.name.toLowerCase().includes(q) ||
                (p.description && p.description.toLowerCase().includes(q)) ||
                (p.tags && p.tags.some((t) => t.toLowerCase().includes(q))) ||
                (p.cid && p.cid.toLowerCase().includes(q)) ||
                (p.folder && p.folder.toLowerCase().includes(q))
            );
        }
        if ($statusFilter === "local") {
            pkgs = pkgs.filter((p) => p.status === "local" || !p.status);
        } else if ($statusFilter === "external") {
            pkgs = pkgs.filter((p) => p.status === "external");
        } else if ($statusFilter === "published") {
            pkgs = pkgs.filter((p) => p.status === "published");
        }
        return groupByFolder(pkgs);
    }
);
