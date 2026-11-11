import test from "node:test";
import assert from "node:assert/strict";
import { createClient, API_VERSION, PercentGradingError } from "../index.mjs";

const token = "test-token-that-is-at-least-thirty-two-characters";
function success(request, result = { lock: { contract_hash: "example" } }, status = 200) {
  return new Response(JSON.stringify({ api_version: API_VERSION, request_id: request.request_id, result }), { status });
}

test("compile uses the versioned envelope and never exposes provider keys", async () => {
  let calls = 0;
  const client = createClient({ token, fetchImpl: async (url, options) => {
    calls += 1;
    assert.equal(url, "http://127.0.0.1:8787/v1/dispatch");
    assert.equal(options.headers.Authorization, `Bearer ${token}`);
    assert.equal(options.redirect, "error");
    const request = JSON.parse(options.body);
    assert.deepEqual(request, { api_version: API_VERSION, request_id: "test-compile", payload: { operation: "compile", source: "example" } });
    return success(request);
  }});
  assert.deepEqual(await client.compile("example", { requestId: "test-compile" }), { lock: { contract_hash: "example" } });
  assert.equal(calls, 1);
});

test("structured server errors retain code and status without retrying", async () => {
  let calls = 0;
  const client = createClient({ token, fetchImpl: async (_, options) => {
    calls += 1;
    const request = JSON.parse(options.body);
    return new Response(JSON.stringify({ api_version: API_VERSION, request_id: request.request_id, error: { code: "live_disabled", message: "Live grading is disabled." } }), { status: 403 });
  }});
  await assert.rejects(client.grade({ lock: {}, answer: "x", config: {} }), error => error instanceof PercentGradingError && error.code === "live_disabled" && error.status === 403);
  assert.equal(calls, 1);
});

test("mismatched request identity, protocol and malformed result are rejected", async () => {
  for (const envelope of [
    { api_version: API_VERSION, request_id: "different", result: {} },
    { api_version: "unknown", request_id: "test", result: {} },
    { api_version: API_VERSION, request_id: "test", result: {}, error: {} },
    { api_version: API_VERSION, request_id: "test" },
  ]) {
    const client = createClient({ token, fetchImpl: async () => new Response(JSON.stringify(envelope)) });
    await assert.rejects(client.compile("x", { requestId: "test" }), error => error.code === "invalid_response");
  }
});

test("invalid endpoint, token, timeout and operation fail before network", async () => {
  assert.throws(() => createClient({ token, baseUrl: "https://user:password@example.org" }));
  assert.throws(() => createClient({ token, baseUrl: "https://example.org/path" }));
  assert.throws(() => createClient({ token: "short" }));
  assert.throws(() => createClient({ token, timeoutMs: 0 }));
  let calls = 0;
  const client = createClient({ token, fetchImpl: async () => { calls += 1; } });
  await assert.rejects(client.request({ operation: "third_provider" }), TypeError);
  await assert.rejects(client.compile("x", { requestId: "has space" }), TypeError);
  await assert.rejects(client.compile("x".repeat(2 * 1024 * 1024)), TypeError);
  assert.equal(calls, 0);
});

test("timeout aborts transport and never automatically retries a grade", async () => {
  let calls = 0;
  let signal;
  const client = createClient({ token, timeoutMs: 5, fetchImpl: async (_, options) => {
    calls += 1; signal = options.signal;
    return new Promise(() => {});
  }});
  await assert.rejects(client.grade({ lock: {}, answer: "x", config: {} }), error => error.code === "timeout");
  assert.equal(calls, 1);
  assert.equal(signal.aborted, true);
});

test("network errors do not leak transport details or token values", async () => {
  const client = createClient({ token, fetchImpl: async () => { throw new Error(`secret ${token}`); } });
  await assert.rejects(client.compile("x"), error => error.code === "network_error" && !error.message.includes(token));
});

test("oversized response is rejected while streaming", async () => {
  const client = createClient({ token, fetchImpl: async () => new Response("x".repeat(2 * 1024 * 1024 + 1)) });
  await assert.rejects(client.compile("x"), error => error.code === "invalid_response");
});

test("HTTP error cannot masquerade as a successful result", async () => {
  const client = createClient({ token, fetchImpl: async (_, options) => success(JSON.parse(options.body), {}, 500) });
  await assert.rejects(client.compile("x"), error => error.code === "http_error" && error.status === 500);
});

test("pre-parse authentication failures may omit request identity", async () => {
  const client = createClient({ token, fetchImpl: async () => new Response(JSON.stringify({
    api_version: API_VERSION, request_id: null, error: { code: "unauthorized", message: "Authentication required." },
  }), { status: 401 }) });
  await assert.rejects(client.compile("x"), error => error.code === "unauthorized" && error.status === 401);
});
