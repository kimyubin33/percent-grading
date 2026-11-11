// Run from any directory after starting the local Rust service.
// This example only compiles a policy and prepares a prompt; it calls no AI API.
import { readFile } from "node:fs/promises";
import { createClient } from "../../sdk/javascript/index.mjs";

const client = createClient({
  baseUrl: process.env.PERCENT_GRADING_URL ?? "http://127.0.0.1:8787",
  token: process.env.PERCENT_GRADING_API_TOKEN,
});
const source = await readFile(new URL("../../policies/apple.aha", import.meta.url), "utf8");
const answer = await readFile(new URL("../rust/apple-red.txt", import.meta.url), "utf8");
const { lock } = await client.compile(source);
const prompt = await client.prompt({ lock, answer });
console.log(JSON.stringify({ contract_hash: lock.contract_hash, prompt_bytes: Buffer.byteLength(prompt.user), schema_type: prompt.schema.type }, null, 2));
