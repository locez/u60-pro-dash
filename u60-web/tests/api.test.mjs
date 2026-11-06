import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { transformSync } from "esbuild";
const source = await readFile(
  new URL("../src/api.ts", import.meta.url),
  "utf8",
);
const code = transformSync(source, {
  loader: "ts",
  format: "esm",
  target: "es2022",
}).code;
const { EventParser, api, setSession, sessionStore } = await import(
  "data:text/javascript;base64," + Buffer.from(code).toString("base64")
);
test("SSE survives arbitrary chunk boundaries and comments, including CRLF", () => {
  const parser = new EventParser();
  assert.deepEqual(parser.push(": keepalive\r\neve"), []);
  assert.deepEqual(
    parser.push(
      'nt: signal\r\ndata: {"data":{"network_provider":"中国联通"}}\r',
    ),
    [],
  );
  assert.deepEqual(parser.push("\n\r\n"), [
    { event: "signal", data: { data: { network_provider: "中国联通" } } },
  ]);
  assert.deepEqual(parser.push('event: bye\ndata: {"reason":"logout"}\n\n'), [
    { event: "bye", data: { reason: "logout" } },
  ]);
  assert.throws(
    () => new EventParser().push("x".repeat(2 * 1024 * 1024 + 1)),
    /too large/,
  );
});
test("session boundaries cancel stale reads, expire credentials and never replay a write", async () => {
  const original = globalThis.fetch;
  try {
    setSession("a".repeat(64));
    let finish;
    globalThis.fetch = async () => ({
      ok: true,
      status: 200,
      json: () => new Promise((resolve) => (finish = resolve)),
    });
    const reading = api("/api/device");
    await Promise.resolve();
    await Promise.resolve();
    setSession("b".repeat(64));
    finish({ ok: true, data: { private: "old session" } });
    await assert.rejects(reading, { name: "AbortError" });
    let calls = 0;
    globalThis.fetch = async () => {
      calls++;
      throw new Error("connection lost");
    };
    await assert.rejects(
      api("/api/wifi/settings", { method: "PUT", body: { wifi_onoff: "0" } }),
      (e) => e.uncertain === true,
    );
    assert.equal(calls, 1);
    globalThis.fetch = async () => ({ status: 401 });
    await assert.rejects(api("/api/device"), (e) => e.status === 401);
    assert.equal(sessionStore.get().token, "");
    setSession("c".repeat(64));
    globalThis.fetch = async () => ({
      ok: false,
      status: 401,
      json: async () => ({ ok: false, error: "Invalid password" }),
    });
    await assert.rejects(
      api("/api/auth/login", {
        method: "POST",
        body: { password: "test-only" },
      }),
    );
    assert.equal(
      sessionStore.get().token,
      "c".repeat(64),
      "failed login must not discard a different session",
    );
  } finally {
    globalThis.fetch = original;
    setSession("");
  }
});
