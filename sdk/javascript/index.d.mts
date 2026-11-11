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
export type Operation =
  | { operation: "compile"; source: string }
  | ({ operation: "prompt" } & AnswerInput)
  | ({ operation: "evaluate" } & EvaluateInput)
  | ({ operation: "grade" } & GradeInput)
  | { operation: "evidence_policy"; lock: LockedContract; source: string }
  | { operation: "evidence_prepare"; lock: LockedContract; policy: Json; dossier: Json }
  | { operation: "evidence_aggregate"; lock: LockedContract; input: Json; openai: Json; claude: Json };
export interface RequestOptions { requestId?: string; }
export interface ClientOptions { baseUrl?: string; token: string; timeoutMs?: number; fetchImpl?: typeof fetch; }
export class PercentGradingError extends Error { code: string; status: number | null; constructor(code: string, message: string, status?: number | null); }
export interface Client {
  request<T = Json>(payload: Operation, options?: RequestOptions): Promise<T>;
  compile(source: string, options?: RequestOptions): Promise<{ lock: LockedContract }>;
  prompt(input: AnswerInput, options?: RequestOptions): Promise<{ system: string; user: string; schema: Json }>;
  evaluate(input: EvaluateInput, options?: RequestOptions): Promise<{ origin: "imported_judgments"; report: Json; evidence_hash: string | null; evidence_assessments: Json }>;
  grade(input: GradeInput, options?: RequestOptions): Promise<Json>;
}
export function createClient(options: ClientOptions): Client;
