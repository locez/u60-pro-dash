export type ObjectData = Record<string, any>;
export class ApiError extends Error {
  constructor(
    message: string,
    public status = 0,
    public details: ObjectData = {},
    public uncertain = false,
  ) {
    super(message);
  }
}
const key = "openu60.session";
type Session = { token: string; epoch: number };
let saved = "";
try {
  saved = sessionStorage.getItem(key) || "";
} catch {
  /* Memory-only session. */
}
let session: Session = {
  token: /^[a-f0-9]{64}$/.test(saved) ? saved : "",
  epoch: 0,
};
const subscribers = new Set<() => void>();
const active = new Set<AbortController>();
export const sessionStore = {
  get: () => session,
  subscribe: (fn: () => void) => {
    subscribers.add(fn);
    return () => {
      subscribers.delete(fn);
    };
  },
};
export function setSession(token: string) {
  for (const request of active) request.abort();
  active.clear();
  session = { token, epoch: session.epoch + 1 };
  try {
    token ? sessionStorage.setItem(key, token) : sessionStorage.removeItem(key);
  } catch {
    /* Memory-only session. */
  }
  subscribers.forEach((fn) => fn());
}
export function bearer(): Record<string, string> {
  return session.token ? { Authorization: `Bearer ${session.token}` } : {};
}
export async function api<T = ObjectData>(
  path: string,
  options: {
    method?: string;
    body?: unknown;
    raw?: BodyInit;
    contentType?: string;
    signal?: AbortSignal;
    timeout?: number;
  } = {},
): Promise<T> {
  if (!path.startsWith("/api/")) throw new Error("Invalid API path");
  const epoch = session.epoch,
    controller = new AbortController();
  active.add(controller);
  const timeout = setTimeout(
    () => controller.abort(),
    options.timeout || 20000,
  );
  const abort = () => controller.abort();
  options.signal?.addEventListener("abort", abort, { once: true });
  if (options.signal?.aborted) controller.abort();
  const writing = (options.method || "GET") !== "GET";
  try {
    const response = await fetch(path, {
      method: options.method || "GET",
      body:
        options.raw ??
        (options.body === undefined ? undefined : JSON.stringify(options.body)),
      cache: "no-store",
      credentials: "omit",
      signal: controller.signal,
      headers: {
        ...bearer(),
        ...(options.raw || options.body !== undefined
          ? { "Content-Type": options.contentType || "application/json" }
          : {}),
      },
    });
    if (epoch !== session.epoch)
      throw new DOMException("Session changed", "AbortError");
    if (response.status === 401 && path !== "/api/auth/login") {
      setSession("");
      throw new ApiError("Session expired", 401);
    }
    const result = await response.json().catch(() => {
      throw new ApiError(
        "Invalid response from router",
        response.status,
        {},
        writing,
      );
    });
    if (epoch !== session.epoch)
      throw new DOMException("Session changed", "AbortError");
    if (!response.ok || result.ok === false)
      throw new ApiError(
        result.error || "Request failed",
        response.status,
        result,
      );
    return result.data ?? result;
  } catch (error) {
    if (error instanceof ApiError) throw error;
    if (options.signal?.aborted || epoch !== session.epoch)
      throw new DOMException("Cancelled", "AbortError");
    throw new ApiError(
      writing
        ? "The change could not be verified. Check the current state before trying again."
        : "Cannot reach the router",
      0,
      {},
      writing,
    );
  } finally {
    clearTimeout(timeout);
    active.delete(controller);
    options.signal?.removeEventListener("abort", abort);
  }
}
export async function login(password: string) {
  const result = await api<{ token: string }>("/api/auth/login", {
    method: "POST",
    body: { password },
  });
  if (!/^[a-f0-9]{64}$/.test(result.token))
    throw new ApiError("Invalid session response");
  setSession(result.token);
}
export async function logout() {
  try {
    await api("/api/auth/logout", { method: "POST", body: {}, timeout: 5000 });
  } finally {
    setSession("");
  }
}

export type Frame = { event: string; data: unknown };
export class EventParser {
  private buffer = "";
  push(chunk: string): Frame[] {
    this.buffer += chunk;
    if (this.buffer.length > 2 * 1024 * 1024)
      throw new Error("Event frame too large");
    const frames: Frame[] = [];
    for (;;) {
      const end = /\r?\n\r?\n/.exec(this.buffer);
      if (!end || end.index === undefined) break;
      const raw = this.buffer.slice(0, end.index);
      this.buffer = this.buffer.slice(end.index + end[0].length);
      let event = "message";
      const data: string[] = [];
      for (const line of raw.split(/\r?\n/)) {
        if (line.startsWith("event:")) event = line.slice(6).trim();
        if (line.startsWith("data:")) data.push(line.slice(5).trimStart());
      }
      if (data.length)
        frames.push({ event, data: JSON.parse(data.join("\n")) });
    }
    return frames;
  }
}
