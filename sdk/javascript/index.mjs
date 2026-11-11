/** Backend/native integration client. Never put provider keys in browser code. */
export const API_VERSION = "percent-grading-api-v1";
const OPERATIONS = new Set([
  "compile", "prompt", "evaluate", "grade", "evidence_policy", "evidence_prepare", "evidence_aggregate",
]);
const MAX_RESPONSE_BYTES = 2 * 1024 * 1024;
let sequence = 0;

export class PercentGradingError extends Error {
  constructor(code, message, status = null) {
    super(message);
    this.name = "PercentGradingError";
    this.code = code;
    this.status = status;
  }
}

function object(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

async function readResponse(response) {
  const declared = response.headers?.get("content-length");
  if (declared && Number(declared) > MAX_RESPONSE_BYTES) {
    await response.body?.cancel();
    throw new PercentGradingError("invalid_response", "The API response exceeds the client size limit.", response.status);
  }
  if (!response.body?.getReader) {
    const value = await response.text();
    if (new TextEncoder().encode(value).length > MAX_RESPONSE_BYTES) {
      throw new PercentGradingError("invalid_response", "The API response exceeds the client size limit.", response.status);
    }
    return value;
  }
  const reader = response.body.getReader();
  const decoder = new TextDecoder("utf-8", { fatal: true });
  let bytes = 0;
  let value = "";
  try {
    for (;;) {
      const chunk = await reader.read();
      if (chunk.done) break;
      bytes += chunk.value.byteLength;
      if (bytes > MAX_RESPONSE_BYTES) {
        await reader.cancel();
        throw new PercentGradingError("invalid_response", "The API response exceeds the client size limit.", response.status);
      }
      value += decoder.decode(chunk.value, { stream: true });
    }
    return value + decoder.decode();
  } finally {
    reader.releaseLock();
  }
}

/** Creates one reusable client. Requests are never automatically retried. */
export function createClient({ baseUrl = "http://127.0.0.1:8787", token, timeoutMs = 180000, fetchImpl = globalThis.fetch } = {}) {
  let url;
  try { url = new URL(baseUrl); } catch { throw new TypeError("baseUrl must be an absolute HTTP(S) URL."); }
  if (!["http:", "https:"].includes(url.protocol) || url.username || url.password || url.search || url.hash) {
    throw new TypeError("baseUrl must use HTTP(S), without credentials, query, or fragment.");
  }
  if (url.pathname !== "/") throw new TypeError("baseUrl must identify the service origin without a path.");
  if (typeof token !== "string" || !/^[\x21-\x7e]{32,256}$/.test(token)) {
    throw new TypeError("token must contain 32..256 visible ASCII characters without whitespace.");
  }
  if (!Number.isInteger(timeoutMs) || timeoutMs < 1 || timeoutMs > 600000) {
    throw new TypeError("timeoutMs must be an integer from 1 to 600000.");
  }
  if (typeof fetchImpl !== "function") throw new TypeError("A fetch implementation is required.");
  const endpoint = new URL("/v1/dispatch", url).href;

  async function request(payload, { requestId = `js-${Date.now().toString(36)}-${(++sequence).toString(36)}` } = {}) {
    if (!object(payload) || !OPERATIONS.has(payload.operation)) throw new TypeError("Unsupported API operation.");
    if (typeof requestId !== "string" || !/^[A-Za-z0-9_.-]{1,128}$/.test(requestId)) {
      throw new TypeError("requestId must contain 1..128 ASCII letters, digits, underscores, dots or hyphens.");
    }
    let body;
    try { body = JSON.stringify({ api_version: API_VERSION, request_id: requestId, payload }); }
    catch { throw new TypeError("The request must be JSON serializable."); }
    if (new TextEncoder().encode(body).length > 2 * 1024 * 1024) throw new TypeError("The request exceeds 2 MiB.");
    const controller = new AbortController();
    let timedOut = false;
    let timer;
    const timeout = new Promise((_, reject) => {
      timer = setTimeout(() => {
        timedOut = true;
        controller.abort();
        reject(new PercentGradingError("timeout", "The API request timed out. Server work may still be running; no retry was sent."));
      }, timeoutMs);
    });
    const transport = (async () => {
      let response;
      try {
        response = await fetchImpl(endpoint, {
          method: "POST", redirect: "error", credentials: "omit",
          headers: { "Content-Type": "application/json", Authorization: `Bearer ${token}` },
          body, signal: controller.signal,
        });
      } catch {
        if (timedOut) throw new PercentGradingError("timeout", "The API request timed out. Server work may still be running; no retry was sent.");
        throw new PercentGradingError("network_error", "The API could not be reached. No retry was sent.");
      }
      let envelope;
      try { envelope = JSON.parse(await readResponse(response)); }
      catch (error) {
        if (error instanceof PercentGradingError) throw error;
        throw new PercentGradingError("invalid_response", "The API did not return a valid JSON response.", response.status);
      }
      const hasResult = Object.hasOwn(envelope ?? {}, "result");
      const hasError = Object.hasOwn(envelope ?? {}, "error");
      const matchingId = envelope?.request_id === requestId || (hasError && envelope?.request_id === null);
      if (!object(envelope) || envelope.api_version !== API_VERSION || !matchingId || hasResult === hasError) {
        throw new PercentGradingError("invalid_response", "The API response does not match the request or protocol.", response.status);
      }
      if (hasError) {
        const error = envelope.error;
        if (!object(error) || typeof error.code !== "string" || typeof error.message !== "string") {
          throw new PercentGradingError("invalid_response", "The API returned a malformed error.", response.status);
        }
        // Only server messages are exposed; transport exceptions and tokens are not.
        throw new PercentGradingError(error.code, error.message, response.status);
      }
      if (!response.ok) throw new PercentGradingError("http_error", "The API returned an unsuccessful HTTP status.", response.status);
      return envelope.result;
    })();
    try { return await Promise.race([transport, timeout]); }
    finally { clearTimeout(timer); }
  }

  return Object.freeze({
    request,
    compile: (source, options) => request({ operation: "compile", source }, options),
    prompt: (input, options) => request({ ...input, operation: "prompt" }, options),
    evaluate: (input, options) => request({ ...input, operation: "evaluate" }, options),
    grade: (input, options) => request({ ...input, operation: "grade" }, options),
  });
}
