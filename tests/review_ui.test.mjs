import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";

const source = readFileSync(new URL("../src/review_helpers.js", import.meta.url), "utf8");
const context = vm.createContext({ TextEncoder });
vm.runInContext(source, context);
const form = context.ExaminerReviewForm;
const template = Object.freeze({ format: "percent-grading-examiner-review-v1", report_hash: "a".repeat(64), contract_hash: "b".repeat(64), answer_hash: "c".repeat(64), expected_decision_hash: "d".repeat(64), human_percent: -1, mode: "balanced", examiner_id: "", grading_basis: "", rationale: "", reviewed_ai_report: false, acknowledged_ai_limits: false });
const report = { engine_version: "0.7.0", status: "ready", current: { percent: 60 } };
const input = () => ({ examiner_id: "examiner_01", human_percent: 80, mode: "balanced", grading_basis: "Assess the stated mechanism and its conditions.", rationale: "The valid mechanism deserves more credit than the AI assigned.", reviewed_ai_report: true, acknowledged_ai_limits: true });

test("balanced preview combines one human and one AI component and exports no final grade", () => {
  assert.equal(form.preview(input(), template, report), 70);
  const output = form.build(input(), template, report);
  assert.equal(output.human_percent, 80);
  assert.equal(output.report_hash, template.report_hash);
  assert.equal(output.expected_decision_hash, template.expected_decision_hash);
  assert.equal("final_percent" in output, false);
  assert.equal("decision_hash" in output, false);
});

test("explicit examiner override works with unresolved or blocked AI and retains the score ceiling", () => {
  const review = { ...input(), mode: "human_override", human_percent: 99.9 };
  assert.equal(form.preview(review, template, { ...report, status: "blocked", current: { percent: null } }), 99.9);
  assert.throws(() => form.build({ ...review, human_percent: 100 }, template, report));
  assert.throws(() => form.build(input(), template, { ...report, status: "review", current: { percent: null } }));
});

test("empty, nonnumeric, nonfinite, missing and out-of-range human scores cannot finalize", () => {
  for (const human_percent of [NaN, Infinity, -1, 100, "80", "", null, undefined]) {
    assert.throws(() => form.build({ ...input(), human_percent }, template, report));
  }
  assert.equal(form.preview({ ...input(), human_percent: 0 }, template, report), 30);
});

test("explicit acknowledgments, bounded reasons and recognized form fields are required", () => {
  for (const patch of [
    { reviewed_ai_report: false }, { acknowledged_ai_limits: false },
    { reviewed_ai_report: "true" }, { grading_basis: " " }, { rationale: "bad\0value" },
    { grading_basis: "한".repeat(6000) }, { examiner_id: "../admin" },
    { mode: "automatic" }, { final_percent: 99.9 }, { expected_decision_hash: "forged" },
  ]) assert.throws(() => form.build({ ...input(), ...patch }, template, report));
});

test("legacy records and absent host templates remain read-only", () => {
  assert.throws(() => form.build(input(), null, report));
  assert.throws(() => form.build(input(), template, { ...report, engine_version: "0.6.0" }));
  const copy = JSON.stringify(template);
  form.build(input(), template, report);
  assert.equal(JSON.stringify(template), copy);
});
