import test from "node:test";
import assert from "node:assert/strict";
import { createClient, API_VERSION, PercentGradingError, officialPercent } from "../index.mjs";

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

test("proof adapters preserve provisional scores without releasing an official grade", async () => {
  const operations = [];
  const result = {
    report: { current: { percent: 99.9 }, decision: { status: "awaiting_examiner", final_percent: null } },
    verification: { checker: "bounded-propositional-checker" },
    celebration: { display_percent: 10000, official_percent: null, message: "Celebration only; examiner review is still required." },
  };
  const client = createClient({ token, fetchImpl: async (_, options) => {
    const request = JSON.parse(options.body);
    operations.push(request.payload.operation);
    return success(request, result);
  }});
  await client.proofPolicy({ lock: {}, source: "challenge" });
  await client.proofPrompt({ lock: {}, policy: {}, answer: "proof" });
  const checked = await client.proofEvaluate({ operation: "grade", lock: {}, policy: {}, answer: "proof", certificate: {}, openai: {}, claude: {} });
  assert.deepEqual(operations, ["proof_policy", "proof_prompt", "proof_evaluate"]);
  assert.deepEqual(checked, result);
  assert.equal(checked.report.current.percent, 99.9);
  assert.equal(officialPercent(checked.report), null);
  assert.equal(checked.celebration.display_percent, 10000);
});

test("a failed benchmark gate is returned for inspection rather than hidden as transport failure", async () => {
  const gate = { passed: false, status: "failed", violations: [{ code: "wrong_settled", scope: "overall", message: "Regression detected." }] };
  const client = createClient({ token, fetchImpl: async (_, options) => {
    const request = JSON.parse(options.body);
    assert.equal(request.payload.operation, "benchmark_gate");
    assert.deepEqual(request.payload.policy, { suite_hash: "frozen" });
    return success(request, { gate });
  }});
  assert.deepEqual(await client.benchmarkGate({ suite: {}, results: {}, policy: { suite_hash: "frozen" } }), { gate });
});

test("invalid proof certificates fail without a retry or an invented celebration", async () => {
  let calls = 0;
  const client = createClient({ token, fetchImpl: async (_, options) => {
    calls += 1;
    const request = JSON.parse(options.body);
    return new Response(JSON.stringify({ api_version: API_VERSION, request_id: request.request_id,
      error: { code: "validation_failed", message: "Certificate rows do not match the challenge." } }), { status: 422 });
  }});
  await assert.rejects(client.proofEvaluate({ lock: {}, policy: {}, answer: "Fermat is proved", certificate: {}, openai: {}, claude: {} }),
    error => error.code === "validation_failed" && error.status === 422);
  assert.equal(calls, 1);
});

test("officialPercent never releases AI-only, legacy, incomplete or malformed final results", () => {
  assert.equal(officialPercent({ current: { percent: 99.9 } }), null);
  assert.equal(officialPercent({ decision: { status: "awaiting_examiner", final_percent: null } }), null);
  const report = { status: "ready", current: { percent: 90 }, decision: {
    status: "finalized", mode: "balanced", human_weight_percent: 50, ai_weight_percent: 50,
    human_percent: 70, ai_percent: 90, final_percent: 80,
    review: { mode: "balanced", human_percent: 70, reviewed_ai_report: true, acknowledged_ai_limits: true },
  } };
  assert.equal(officialPercent(report), 80);
  for (const change of [ { final_percent: 90 }, { human_weight_percent: 0 }, { final_percent: 100 }, { final_percent: NaN } ]) {
    assert.throws(() => officialPercent({ ...report, decision: { ...report.decision, ...change } }), TypeError);
  }
  assert.throws(() => officialPercent({ ...report, status: "blocked" }), TypeError);
  assert.throws(() => officialPercent({ ...report, decision: { ...report.decision, review: { ...report.decision.review, reviewed_ai_report: "true" } } }), TypeError);
  assert.equal(officialPercent({ ...report, status: "blocked", decision: { ...report.decision,
    mode: "human_override", human_weight_percent: 100, ai_weight_percent: 0, final_percent: 70,
    review: { ...report.decision.review, mode: "human_override" },
  } }), 70);
});

test("review adapters send explicit examiner operations without impersonating completion", async () => {
  const operations = [];
  const client = createClient({ token, fetchImpl: async (_, options) => {
    const request = JSON.parse(options.body);
    operations.push(request.payload.operation);
    if (request.payload.operation === "examiner_review") assert.equal(request.payload.review.reviewed_ai_report, false);
    return success(request, { status: "fixture_only" });
  } });
  await client.reviewTemplate({ lock: {}, answer: "A", report: {} });
  await client.reviewReport({ lock: {}, answer: "A", report: {} });
  await client.examinerReview({ operation: "grade", lock: {}, answer: "A", report: {}, review: { reviewed_ai_report: false } });
  assert.deepEqual(operations, ["review_template", "review_report", "examiner_review"]);
});
