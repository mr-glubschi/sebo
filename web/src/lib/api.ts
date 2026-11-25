import type { Bookmark } from "./bindings/Bookmark";
import type { Folder } from "./bindings/Folder";
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
  const response = await fetch(`/api${path}`, {
    method,
    headers: body === undefined ? {} : { "Content-Type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });

  if (!response.ok) {
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

  if (response.status === 204) return undefined as T;
  return (await response.json()) as T;
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
}

export interface FolderChanges {
  name?: string;
  parent_id?: number | null;
}

/** `move`: Inhalt in den übergeordneten Ordner, `delete`: mitlöschen. */
export type DeleteContents = "move" | "delete";

export const api = {
  status: () => request<AuthStatus>("GET", "/auth/status"),
  login: (password: string, code: string) =>
    request<LoginResult>("POST", "/auth/login", { password, code }),
  logout: () => request<void>("POST", "/auth/logout"),

  library: () => request<Library>("GET", "/library"),

  createFolder: (name: string, parent_id: number | null) =>
    request<Folder>("POST", "/folders", { name, parent_id }),
  updateFolder: (id: number, changes: FolderChanges) =>
    request<Folder>("PATCH", `/folders/${id}`, changes),
  deleteFolder: (id: number, contents: DeleteContents) =>
    request<void>("DELETE", `/folders/${id}?contents=${contents}`),

  createBookmark: (bookmark: NewBookmark) => request<Bookmark>("POST", "/bookmarks", bookmark),
  updateBookmark: (id: number, changes: BookmarkChanges) =>
    request<Bookmark>("PATCH", `/bookmarks/${id}`, changes),
  deleteBookmark: (id: number) => request<void>("DELETE", `/bookmarks/${id}`),
  refreshBookmark: (id: number) => request<Bookmark>("POST", `/bookmarks/${id}/refresh`),
  removeImage: (id: number) => request<Bookmark>("DELETE", `/bookmarks/${id}/image`),
  preview: (url: string) => request<Preview>("POST", "/preview", { url }),
};
