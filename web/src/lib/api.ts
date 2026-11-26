import type { Bookmark } from "./bindings/Bookmark";
import type { Folder } from "./bindings/Folder";
import type { ImportResult } from "./bindings/ImportResult";
import type { Library } from "./bindings/Library";
import type { Preview } from "./bindings/Preview";

export class ApiError extends Error {
  constructor(
    message: string,
    readonly status: number,
    readonly retryAfter?: number,
  ) {
    super(message);
  }
}

let unauthorizedHandler: (() => void) | undefined;

/** Wird aufgerufen, wenn die Sitzung abgelaufen ist (401 außerhalb des Logins). */
export function onUnauthorized(handler: () => void) {
  unauthorizedHandler = handler;
}

async function request<T>(method: string, path: string, body?: unknown): Promise<T> {
  const raw = body instanceof Blob;
  const response = await fetch(`/api${path}`, {
    method,
    headers:
      body === undefined ? {} : { "Content-Type": raw ? "text/html" : "application/json" },
    body: body === undefined ? undefined : raw ? body : JSON.stringify(body),
  });

  if (!response.ok) await fail(response, path);
  if (response.status === 204) return undefined as T;
  return (await response.json()) as T;
}

async function fail(response: Response, path: string): Promise<never> {
  let message = `Fehler ${response.status}`;
  let retryAfter: number | undefined;
  try {
    const data = (await response.json()) as { error?: string; retry_after?: number };
    if (data.error) message = data.error;
    retryAfter = data.retry_after;
  } catch {
    // Antwort ohne JSON
  }
  if (response.status === 401 && !path.startsWith("/auth/")) unauthorizedHandler?.();
  throw new ApiError(message, response.status, retryAfter);
}

/**
 * Lädt die Bibliothek. Mit dem ETag des letzten Stands antwortet der Server
 * bei unveränderten Daten nur mit 304; dann kommt `null` zurück.
 */
async function loadLibrary(etag?: string): Promise<{ library: Library; etag?: string } | null> {
  const response = await fetch("/api/library", {
    headers: etag ? { "If-None-Match": etag } : {},
  });
  if (response.status === 304) return null;
  if (!response.ok) await fail(response, "/library");
  return {
    library: (await response.json()) as Library,
    etag: response.headers.get("ETag") ?? undefined,
  };
}

/** Fehlermeldung für die Anzeige. */
export function errorText(error: unknown): string {
  return error instanceof ApiError ? error.message : "Server nicht erreichbar";
}

export interface AuthStatus {
  authenticated: boolean;
  setup_required: boolean;
}

export interface LoginResult {
  recovery_codes?: string[];
}

export interface NewBookmark {
  url: string;
  title?: string;
  folder_id: number | null;
}

export interface BookmarkChanges {
  url?: string;
  /** Leer = Titel wieder automatisch von der Seite übernehmen. */
  title?: string;
  folder_id?: number | null;
  favorite?: boolean;
}

export interface FolderChanges {
  name?: string;
  parent_id?: number | null;
  /** `null` = Standard-Symbol. */
  icon?: string | null;
  /** `null` = Akzentfarbe. */
  color?: string | null;
}

export type BulkAction =
  | { action: "move"; folder_id: number | null }
  | { action: "delete" }
  | { action: "favorite" }
  | { action: "unfavorite" };

/** `move`: Inhalt in den übergeordneten Ordner, `delete`: mitlöschen. */
export type DeleteContents = "move" | "delete";

export const api = {
  status: () => request<AuthStatus>("GET", "/auth/status"),
  login: (password: string, code: string) =>
    request<LoginResult>("POST", "/auth/login", { password, code }),
  logout: () => request<void>("POST", "/auth/logout"),

  library: loadLibrary,

  createFolder: (name: string, parent_id: number | null) =>
    request<Folder>("POST", "/folders", { name, parent_id }),
  updateFolder: (id: number, changes: FolderChanges) =>
    request<Folder>("PATCH", `/folders/${id}`, changes),
  deleteFolder: (id: number, contents: DeleteContents) =>
    request<void>("DELETE", `/folders/${id}?contents=${contents}`),
  deleteFolders: (ids: number[], contents: DeleteContents) =>
    request<void>("POST", "/folders/bulk-delete", { ids, contents }),
  /** Reihenfolge einer Ebene; Ordner aus anderen Ebenen werden dorthin verschoben. */
  orderFolders: (parentId: number | null, ids: number[]) =>
    request<void>("POST", "/folders/order", { parent_id: parentId, ids }),
  mergeFolders: (target: number, sources: number[]) =>
    request<Folder>("POST", "/folders/merge", { target, sources }),

  createBookmark: (bookmark: NewBookmark) => request<Bookmark>("POST", "/bookmarks", bookmark),
  updateBookmark: (id: number, changes: BookmarkChanges) =>
    request<Bookmark>("PATCH", `/bookmarks/${id}`, changes),
  deleteBookmark: (id: number) => request<void>("DELETE", `/bookmarks/${id}`),
  bulkBookmarks: (ids: number[], action: BulkAction) =>
    request<{ changed: number }>("POST", "/bookmarks/bulk", { ids, ...action }),
  refreshBookmark: (id: number) => request<Bookmark>("POST", `/bookmarks/${id}/refresh`),
  removeImage: (id: number) => request<Bookmark>("DELETE", `/bookmarks/${id}/image`),
  preview: (url: string) => request<Preview>("POST", "/preview", { url }),
  importFile: (file: File) => request<ImportResult>("POST", "/import", file),
};
