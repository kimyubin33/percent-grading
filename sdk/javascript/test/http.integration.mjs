// Actual local TCP integration, run on both GitHub Actions operating systems.
// The server has live calls disabled and receives no provider credentials.
import test from "node:test";
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { readFile } from "node:fs/promises";
import { createHash, randomBytes } from "node:crypto";
import { resolve } from "node:path";
import { createClient } from "../index.mjs";

test("JavaScript client uses the real Rust HTTP service without provider calls", { timeout: 30000 }, async () => {
  const executable = resolve("target/debug", process.platform === "win32" ? "percent-grading.exe" : "percent-grading");
  const token = randomBytes(32).toString("hex");
  const env = { ...process.env, PERCENT_GRADING_API_TOKEN: token };
  delete env.OPENAI_API_KEY;
  delete env.ANTHROPIC_API_KEY;
  const child = spawn(executable, ["serve", "--bind", "127.0.0.1:0"], {
    env, windowsHide: true, stdio: ["ignore", "ignore", "pipe"],
  });
  const exited = new Promise(resolveExit => child.once("exit", resolveExit));
  try {
    const baseUrl = await new Promise((resolveUrl, reject) => {
      const timer = setTimeout(() => reject(new Error("Local Rust server did not become ready.")), 10000);
      let text = "";
      child.stderr.on("data", chunk => {
        text = (text + chunk.toString("utf8")).slice(-4096);
        const found = text.match(/listening at (http:\/\/127\.0\.0\.1:\d+)/);
        if (found) { clearTimeout(timer); resolveUrl(found[1]); }
      });
      child.once("error", () => { clearTimeout(timer); reject(new Error("Could not launch the Rust server.")); });
      child.once("exit", () => { clearTimeout(timer); reject(new Error("Rust server exited before readiness.")); });
    });
    const health = await (await fetch(`${baseUrl}/health`)).json();
    assert.equal(health.status, "ok");
    const client = createClient({ baseUrl, token, timeoutMs: 5000 });
    const source = await readFile(new URL("../../../policies/apple.aha", import.meta.url), "utf8");
    const { lock } = await client.compile(source);
    const fixture = JSON.parse(await readFile(new URL("../../../examples/rust/apple-blue.fixture.json", import.meta.url), "utf8"));
    const prompt = await client.prompt({ lock, answer: fixture.answer });
    assert.equal(JSON.parse(prompt.user).answer, fixture.answer);
    const judgment = rows => ({ contract_hash: lock.contract_hash, answer_hash: createHash("sha256").update(fixture.answer).digest("hex"), judgments: rows });
    const evaluated = await client.evaluate({ lock, answer: fixture.answer, openai: judgment(fixture.openai), claude: judgment(fixture.claude) });
    assert.equal(evaluated.report.status, "blocked");
    assert.equal(evaluated.report.current.percent, 46.25);
    await assert.rejects(client.grade({ lock, answer: fixture.answer, config: {
      openai_model: "offline-openai", claude_model: "offline-claude",
      limits: { timeout_seconds: 10, max_output_tokens: 1024, max_response_bytes: 2097152, max_attempts: 1 },
    } }), error => error.code === "live_disabled");
    const rejected = createClient({ baseUrl, token: "wrong-token-abcdefghijklmnopqrstuvwxyz", timeoutMs: 5000 });
    await assert.rejects(rejected.compile(source), error => error.code === "unauthorized");
  } finally {
    child.kill();
    let timer;
    await Promise.race([exited, new Promise(resolveTimeout => { timer = setTimeout(resolveTimeout, 5000); })]);
    clearTimeout(timer);
  }
});
