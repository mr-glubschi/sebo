export class ApiError extends Error {
  constructor(
    message: string,
    readonly status: number,
    readonly retryAfter?: number,
  ) {
    super(message);
  }
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
    throw new ApiError(message, response.status, retryAfter);
  }

  if (response.status === 204) return undefined as T;
  return (await response.json()) as T;
}

export interface AuthStatus {
  authenticated: boolean;
  setup_required: boolean;
}

export interface LoginResult {
  recovery_codes?: string[];
}

export const api = {
  status: () => request<AuthStatus>("GET", "/auth/status"),
  login: (password: string, code: string) =>
    request<LoginResult>("POST", "/auth/login", { password, code }),
  logout: () => request<void>("POST", "/auth/logout"),
};
