/* Local preview helpers. The Rust examiner-review operation remains authoritative. */
globalThis.ExaminerReviewForm = (() => {
  const fields = ["examiner_id", "human_percent", "mode", "grading_basis", "rationale", "reviewed_ai_report", "acknowledged_ai_limits"];
  function boundedText(value, label) {
    if (typeof value !== "string" || !value.trim() || value.includes("\0") || new TextEncoder().encode(value).length > 16384) {
      throw new Error(`${label} must contain 1–16384 UTF-8 bytes and no NUL character.`);
    }
  }
  function validate(input, template, report) {
    if (!input || Object.keys(input).length !== fields.length || Object.keys(input).some(key => !fields.includes(key))) {
      throw new Error("Provide only the required examiner form fields.");
    }
    if (!template || !report || report.engine_version !== "0.7.0") {
      throw new Error("This record is read-only. Generate a supported review template through the grading host.");
    }
    if (typeof input.examiner_id !== "string" || !/^[A-Za-z0-9_-]{1,64}$/.test(input.examiner_id)) {
      throw new Error("Use an examiner ID with 1–64 ASCII letters, numbers, underscores, or hyphens.");
    }
    if (typeof input.human_percent !== "number" || !Number.isFinite(input.human_percent) || input.human_percent < 0 || input.human_percent > 99.9) {
      throw new Error("Enter your own score from 0 through 99.9. An empty score is not zero.");
    }
    if (input.mode !== "balanced" && input.mode !== "human_override") {
      throw new Error("Choose the 50:50 decision or explicitly choose 100% examiner authority.");
    }
    const ai = report.current?.percent;
    if (input.mode === "balanced" && (report.status !== "ready" || typeof ai !== "number" || !Number.isFinite(ai) || ai < 0 || ai > 99.9)) {
      throw new Error("The AI assessment is unresolved or blocked. A 50:50 final grade is unavailable; an examiner may explicitly override it with a documented judgment.");
    }
    boundedText(input.grading_basis, "Your grading method");
    boundedText(input.rationale, "Your decision rationale");
    if (input.reviewed_ai_report !== true || input.acknowledged_ai_limits !== true) {
      throw new Error("Read the AI explanations and acknowledge both review statements before exporting.");
    }
    return input;
  }
  function build(input, template, report) {
    validate(input, template, report);
    // Binding fields always originate in the host-generated template, not form input.
    return { ...template, ...input };
  }
  function preview(input, template, report) {
    validate(input, template, report);
    return input.mode === "human_override" ? input.human_percent : (input.human_percent + report.current.percent) / 2;
  }
  return Object.freeze({ build, preview });
})();
