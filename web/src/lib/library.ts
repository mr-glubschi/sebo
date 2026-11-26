// Hilfsfunktionen für Ordnerbaum, Routen, Sortierung und Einstellungen.

import type { Bookmark } from "./bindings/Bookmark";
import type { Folder } from "./bindings/Folder";

export type Route =
  | { kind: "favorites" }
  | { kind: "all" }
  | { kind: "unsorted" }
  | { kind: "folder"; id: number };

/** Unbekannte Adressen führen zur Startseite (Favoriten). */
export function parseRoute(path: string): Route {
  if (path === "/alle") return { kind: "all" };
  if (path === "/unsortiert") return { kind: "unsorted" };
  const match = /^\/f\/(\d{1,15})$/.exec(path);
  if (match) return { kind: "folder", id: Number(match[1]) };
  return { kind: "favorites" };
}

export function routePath(route: Route): string {
  switch (route.kind) {
    case "favorites":
      return "/";
    case "all":
      return "/alle";
    case "unsorted":
      return "/unsortiert";
    case "folder":
      return `/f/${route.id}`;
  }
}

export function sameRoute(a: Route, b: Route): boolean {
  return routePath(a) === routePath(b);
}

export interface Tree {
  byId: Map<number, Folder>;
  /** Unterordner je Ordner, `null` = oberste Ebene. Nach Namen sortiert. */
  children: Map<number | null, Folder[]>;
}

const collator = new Intl.Collator("de", { sensitivity: "base", numeric: true });

export function buildTree(folders: Folder[]): Tree {
  const byId = new Map(folders.map((folder) => [folder.id, folder]));
  const children = new Map<number | null, Folder[]>();
  for (const folder of folders) {
    // Unbekannter Elternordner: oben einsortieren statt verschwinden lassen.
    const parent = folder.parent_id !== null && byId.has(folder.parent_id) ? folder.parent_id : null;
    const list = children.get(parent) ?? [];
    list.push(folder);
    children.set(parent, list);
  }
  for (const list of children.values()) list.sort((a, b) => collator.compare(a.name, b.name));
  return { byId, children };
}

/** Der Ordner und alle seine Unterordner. */
export function subtree(tree: Tree, id: number): Set<number> {
  const result = new Set<number>();
  const stack = [id];
  while (stack.length > 0) {
    const current = stack.pop()!;
    if (result.has(current)) continue;
    result.add(current);
    for (const child of tree.children.get(current) ?? []) stack.push(child.id);
  }
  return result;
}

/** Pfad von der obersten Ebene bis zum Ordner. */
export function folderPath(tree: Tree, id: number): Folder[] {
  const path: Folder[] = [];
  let current = tree.byId.get(id);
  while (current && path.length < 100) {
    path.unshift(current);
    current = current.parent_id === null ? undefined : tree.byId.get(current.parent_id);
  }
  return path;
}

export interface FolderOption {
  id: number;
  name: string;
  depth: number;
}

/** Alle Ordner in Baumreihenfolge, z. B. für eine Auswahlliste. */
export function folderOptions(tree: Tree, exclude: Set<number> = new Set()): FolderOption[] {
  const result: FolderOption[] = [];
  const walk = (parent: number | null, depth: number) => {
    for (const folder of tree.children.get(parent) ?? []) {
      if (exclude.has(folder.id)) continue;
      result.push({ id: folder.id, name: folder.name, depth });
      walk(folder.id, depth + 1);
    }
  };
  walk(null, 0);
  return result;
}

export type ViewMode = "cards" | "list" | "compact";
export type SortMode = "new" | "old" | "name";

export const VIEW_MODES: ViewMode[] = ["cards", "list", "compact"];
export const SORT_MODES: SortMode[] = ["new", "old", "name"];

export function sortBookmarks(bookmarks: Bookmark[], mode: SortMode): Bookmark[] {
  const sorted = [...bookmarks];
  switch (mode) {
    case "new":
      return sorted.sort((a, b) => b.created_at - a.created_at || b.id - a.id);
    case "old":
      return sorted.sort((a, b) => a.created_at - b.created_at || a.id - b.id);
    case "name":
      return sorted.sort((a, b) => collator.compare(a.title, b.title) || a.id - b.id);
  }
}

export function host(url: string): string {
  try {
    return new URL(url).hostname.replace(/^www\./, "");
  } catch {
    return url;
  }
}

/** Gleichbleibender Farbton für den Buchstaben-Platzhalter. */
export function hue(text: string): number {
  let hash = 0;
  for (const char of text) hash = (hash * 31 + char.codePointAt(0)!) % 360;
  return hash;
}

/** Gespeicherte Einstellung lesen. Ohne Speicher (privates Fenster) gilt der Standard. */
export function loadPref<T extends string>(key: string, allowed: readonly T[], fallback: T): T {
  try {
    const value = localStorage.getItem(`sebo.${key}`);
    return allowed.find((option) => option === value) ?? fallback;
  } catch {
    return fallback;
  }
}

export function savePref(key: string, value: string) {
  try {
    localStorage.setItem(`sebo.${key}`, value);
  } catch {
    // ohne Speicher eben nicht
  }
}
