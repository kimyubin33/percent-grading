export const API_VERSION: "percent-grading-api-v1";
export type Json = null | boolean | number | string | Json[] | { [key: string]: Json };
export interface LockedContract { format: string; compiler_revision: string; source: string; contract: Json; contract_hash: string; }
export interface JudgmentRow { criterion_id: string; anchor_id: string; evidence: string[]; reason: string; conditions: string[]; }
export interface Judgment { contract_hash: string; answer_hash: string; judgments: JudgmentRow[]; }
/** Checked by Rust on receipt; keep the complete pack returned by evidence_aggregate. */
export interface EvidencePack { format: string; input: Json; openai: Json; claude: Json; claims: Json[]; blocked_criteria: string[]; pack_hash: string; }
export interface Limits { timeout_seconds: number; max_output_tokens: number; max_response_bytes: number; max_attempts: number; }
export interface RunConfig { openai_model: string; claude_model: string; limits: Limits; }
export interface AnswerInput { lock: LockedContract; answer: string; evidence?: EvidencePack | null; }
export interface EvaluateInput extends AnswerInput { openai: Judgment; claude: Judgment; }
export interface GradeInput extends AnswerInput { config: RunConfig; }
/** Provisional AI profile. Never export this as the official examiner-approved grade. */
export interface ScoreProfile { percent: number | null; coverage_percent: number; lower_percent: number; upper_percent: number; }
export interface GradingReport {
  engine_version: string; contract_hash: string; answer_hash: string; status: string;
  current: ScoreProfile; future: ScoreProfile | null; disagreements: string[];
  critical_failures: string[]; criteria: Json[]; interpretation: string; decision?: ExaminerDecision | null;
}
export interface ExaminerReview {
  format: "percent-grading-examiner-review-v1"; report_hash: string; expected_decision_hash: string;
  contract_hash: string; answer_hash: string; examiner_id: string; human_percent: number;
  mode: "balanced" | "human_override"; grading_basis: string; rationale: string;
  reviewed_ai_report: boolean; acknowledged_ai_limits: boolean;
}
export interface ExaminerDecision {
  status: "awaiting_examiner" | "finalized"; mode: "balanced" | "human_override" | null;
  report_hash: string; ai_weight_percent: number; human_weight_percent: number;
  ai_percent: number | null; human_percent: number | null; final_percent: number | null;
  review: ExaminerReview | null; history: ExaminerReview[]; decision_hash: string; notice: string;
}
export interface ReviewInput { lock: LockedContract; answer: string; report: GradingReport; }
export interface ExaminerReviewInput extends ReviewInput { review: ExaminerReview; }
/** Keep the full policy object returned by proofPolicy; Rust recompiles and verifies it. */
export interface LockedProofPolicy {
  format: "percent-grading-proof-policy-v1"; compiler_revision: string; contract_hash: string;
  source: string; policy: { id: string; version: string; criterion: string; variables: string[]; premises: string[]; conclusion: string; };
  policy_hash: string;
}
export interface CertificateRow { assignment: number; premises: boolean[]; conclusion: boolean; }
export interface Certificate { format: "percent-grading-truth-table-v1"; policy_hash: string; answer_hash: string; answer_excerpt: string; rows: CertificateRow[]; }
export interface ProofPolicyInput { lock: LockedContract; source: string; }
export interface ProofPromptInput { lock: LockedContract; policy: LockedProofPolicy; answer: string; }
export interface ProofEvaluateInput extends EvaluateInput { policy: LockedProofPolicy; certificate: Certificate; }
/** Cosmetic metadata only: never add display_percent to a score, average or ranking. */
export interface Celebration { display_percent: 10000; official_percent: number | null; message: string; }
export interface ProofResult {
  origin: "imported_judgments_and_certificate";
  evidence_hash: string | null;
  report: GradingReport;
  verification: { checker: string; policy_hash: string; answer_hash: string; checked_assignments: number; applicable_assignments: number; scope: string; };
  celebration: Celebration | null;
}
export interface GatePolicy {
  format: "percent-grading-benchmark-gate-v1"; id: string; version: string; suite_hash: string;
  mode: "development_smoke" | "release_candidate"; minimum_cases: number; minimum_repeats: number;
  minimum_coverage_percent: number; minimum_accepted_accuracy_percent: number;
  max_wrong_settled: number; max_wrong_agreements: number; max_relation_failures: number;
  max_relation_indeterminate: number; required_categories: { name: string; minimum_cases: number; }[];
}
export interface BenchmarkGateInput { suite: Json; results: Json; policy: GatePolicy; }
/** A failed gate is a valid result, not an HTTP error. Origin labels do not authenticate API execution. */
export interface GateReport {
  format: "percent-grading-benchmark-gate-report-v1"; engine_version: string;
  passed: boolean; status: "passed" | "failed"; policy: GatePolicy; policy_hash: string;
  unique_cases: number; cases_by_category: Record<string, number>;
  benchmark: Json; violations: { scope: string; code: string; message: string; }[];
  interpretation: string; report_hash: string;
}
export type Operation =
  | { operation: "compile"; source: string }
  | ({ operation: "prompt" } & AnswerInput)
  | ({ operation: "evaluate" } & EvaluateInput)
  | ({ operation: "grade" } & GradeInput)
  | { operation: "evidence_policy"; lock: LockedContract; source: string }
  | { operation: "evidence_prepare"; lock: LockedContract; policy: Json; dossier: Json }
  | { operation: "evidence_aggregate"; lock: LockedContract; input: Json; openai: Json; claude: Json }
  | ({ operation: "proof_policy" } & ProofPolicyInput)
  | ({ operation: "proof_prompt" } & ProofPromptInput)
  | ({ operation: "proof_evaluate" } & ProofEvaluateInput)
  | ({ operation: "benchmark_gate" } & BenchmarkGateInput)
  | ({ operation: "review_template" } & ReviewInput)
  | ({ operation: "review_report" } & ReviewInput)
  | ({ operation: "examiner_review" } & ExaminerReviewInput);
export interface RequestOptions { requestId?: string; }
export interface ClientOptions { baseUrl?: string; token: string; timeoutMs?: number; fetchImpl?: typeof fetch; }
export class PercentGradingError extends Error { code: string; status: number | null; constructor(code: string, message: string, status?: number | null); }
export interface Client {
  request<T = Json>(payload: Operation, options?: RequestOptions): Promise<T>;
  compile(source: string, options?: RequestOptions): Promise<{ lock: LockedContract }>;
  prompt(input: AnswerInput, options?: RequestOptions): Promise<{ system: string; user: string; schema: Json }>;
  evaluate(input: EvaluateInput, options?: RequestOptions): Promise<{ origin: "imported_judgments"; report: GradingReport; evidence_hash: string | null; evidence_assessments: Json }>;
  grade(input: GradeInput, options?: RequestOptions): Promise<Json>;
  proofPolicy(input: ProofPolicyInput, options?: RequestOptions): Promise<{ policy: LockedProofPolicy }>;
  proofPrompt(input: ProofPromptInput, options?: RequestOptions): Promise<{ system: string; user: string; schema: Json }>;
  proofEvaluate(input: ProofEvaluateInput, options?: RequestOptions): Promise<ProofResult>;
  benchmarkGate(input: BenchmarkGateInput, options?: RequestOptions): Promise<{ gate: GateReport }>;
  reviewTemplate(input: ReviewInput, options?: RequestOptions): Promise<{ review: ExaminerReview; notice: string }>;
  reviewReport(input: ReviewInput, options?: RequestOptions): Promise<{ html: string; markdown: string; notice: string }>;
  examinerReview(input: ExaminerReviewInput, options?: RequestOptions): Promise<{ report: GradingReport; notice: string }>;
}
export function createClient(options: ClientOptions): Client;
/** Returns null for provisional/legacy data; use only with a Rust-validated report. */
export function officialPercent(report: GradingReport): number | null;
