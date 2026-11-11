# What earlier automated graders teach Percent Grading

Automated grading did not begin with chatbots, and its history is not a sequence of complete failures. Statistical prediction, natural-language processing, machine learning, test execution, and formal analysis have all produced useful systems within particular tasks. The recurring problem is treating success on a limited measurement procedure as evidence of unrestricted understanding.

This review uses eight original research sources. It separates reported findings from engineering decisions for Percent Grading. The sources were checked on 2026-11-11. Their historical experiments are not measurements of current OpenAI or Claude models.

## A history of the problem, not a history of universal failure

Ellis Page's 1967 account describes Project Essay Grade beginning in December 1964. It explicitly distinguishes the qualities a teacher wants to measure from computable proxies for those qualities, and describes multiple regression used to predict human ratings. The distinction between reproducing a score and understanding an answer was already visible in that work. These systems predate modern generative AI; later automated essay scorers already used NLP and machine learning. [1]

ETS researchers subsequently investigated deliberate attempts to fool e-rater. Their 2001 report identifies ways automated evaluation can fail when used as the sole scorer in consequential assessments. This was research by the system's own developers, not evidence that automated scoring has no useful role. [2]

The main engineering lesson is to make the target of assessment explicit, test deliberate counterexamples, and preserve uncertainty and review paths. Replacing a statistical scorer with two language models changes the failure surface; it does not remove it.

## Failure mechanisms and our response

| Mechanism | What the research establishes | Percent Grading response | Remaining limitation |
|---|---|---|---|
| A proxy replaces the intended skill | Page distinguishes measurable proxies from intended qualities. Chodorow and Burstein found that e-rater99 explained little beyond essay length, while e-rater01 improved and depended less on length. [1, 3] | The frozen contract names each criterion and its allowed anchors. Content-only criteria must not reward length, vocabulary, or fluency by themselves. | A clearly written rubric can still measure the wrong thing. A model may still infer the wrong anchor. |
| Fluent or keyword-rich nonsense earns credit | Kabra et al. tested five essay scorers and found that unrelated additions often failed to reduce scores and could raise them. Ding et al. found that word shuffling could fool short-answer content scorers. [4, 5] | Compare correct answers with filler, shuffled keywords, contradictory claims, and persuasive but unsupported conclusions. Preserve correct fragments only where their particular criterion is met. | Exact quoted substrings establish what was written, not whether it is coherent or true. |
| An aggregate agreement measure hides serious errors | The robustness study argues that agreement statistics alone overlook coherence, relevance, and other dimensions. [4] | Report accepted errors, withheld judgments, wrong agreement between both providers, and transformation failures separately. Report coverage beside accepted-answer accuracy. | These metrics are only as useful as the authored expectations and sampled tasks. |
| Biased labels become automated consistency | Uto and Okano examine neural essay scoring trained on ratings affected by rater characteristics and propose an IRT-based mitigation. [6] | Version and inspect rubric expectations; retain appeals and common cohort reassessment when evidence changes. Use independent adjudication before making educational accuracy claims. | Two models can share a misconception. Their agreement is not independent confirmation of truth or fairness. |
| A program passes the tests but is still wrong | Clegg et al. show that mutation analysis can reveal gaps in grading tests; their study used 197 student Java classes across three assignments. [7] | Treat near-correct counterexamples as required regression cases. Test boundary conditions and altered assumptions, not only ideal examples. | A finite benchmark does not establish correctness for arbitrary unseen answers. |
| A formal tool is trusted outside its actual scope | Liu et al. found errors missed by test-based program judges, but also describe solver, symbolic-execution, and supported-operation limits. [8] | A checked proof result must name its formal statement, assumptions, checker, and supported domain. An unsupported claim or incomplete certificate must not unlock a verified achievement. | Verifying a formal statement does not establish that it faithfully represents a natural-language question or the world. |

These responses are engineering choices motivated by the literature. The cited studies do not evaluate Percent Grading or demonstrate that these choices have eliminated the problems.

## Why a good fragment does not rescue every conclusion

Suppose a task declares that a particular apple is observed under ordinary illumination and its supplied label is red. A response may correctly explain that observed color depends on illumination while incorrectly concluding that this apple is blue. An explanation can contain a valid principle and an unsupported application at the same time.

The intent criterion, principle criterion, reasoning criterion, and stated conditions therefore need separate anchors. A valid conditional alternative can receive its declared partial credit without changing the task's assumptions after seeing the answer. A speculative future possibility belongs in a separate future profile. Merely mentioning optics, a famous theorem, or a prestigious institution is not an additional proof step.

This is also why awarding zero to every answer containing a suspicious word would be a poor defense. A learner can quote a false claim in order to refute it; a short or grammatically imperfect answer can fully satisfy a content criterion. Defenses must test meaning in the declared task, not replace one superficial proxy with another.

## A regression protocol that exposes failure

Use the [benchmark harness](BENCHMARK_GUIDE.md) to compare paired answers for the same criterion. Keep a valid answer and a deliberately changed counterpart. Choose the relation before asking either provider to judge.

| Transformation | Expected relation for a content criterion | What must be reviewed |
|---|---|---|
| Restate the same answer concisely or with harmless grammar differences | Equal when meaning and required reasoning are preserved | The task must not assess writing quality or require an explanation that the shorter answer removes. |
| Add irrelevant fluent filler to an already wrong answer | No higher | Extra text may contain a genuinely new valid step; inspect the entire addition. |
| Shuffle relevant keywords into a non-answer | No higher than the corresponding unsupported answer | Keywords are not assertions or relations. |
| Negate the decisive statement or introduce an unresolved contradiction | No higher; apply the specific lower anchor when warranted | Quoting and correcting a misconception is different from endorsing both claims. |
| Add an unsupported claim that an authority or AI proved the answer | Equal to the same underlying answer without that claim | An actual independently checked certificate is a different input, not an authority claim. |
| Change a boundary condition while retaining a memorized conclusion | Lower where the conclusion no longer follows | The new condition must be explicit and relevant. |
| Give a valid alternative derivation | Equal for the satisfied criteria | Do not require one exact wording or one reference solution's surface form. |

Version 0.6 adds the [historical-failure regression corpus](../benchmarks/adversarial/README.md): three public development tasks, 24 answers, and 48 criterion expectations. The tasks cover exact addition, Boolean disjunction, and Python negative indexing. Each task has a baseline and variants for irrelevant padding, grammar, keyword shuffling, negation, contradiction, authority claims, and prompt injection. These are new authored cases inspired by failure mechanisms, not copied study data or replications of the cited experiments.

The `benchmark-gate` command checks a frozen policy against a complete benchmark result. It keeps overall and category-level failures visible and prevents an average from hiding a failed required check. A gate can reject a candidate whose two providers agree on an unacceptable anchor or whose transformation checks fail. Thresholds and the suite must be declared before judging; changing either creates a different evaluation. Imported result labels are not provider signatures.

Keep all changed versions and their expected anchors in the suite. Do not silently revise the expected answer after seeing which result a preferred model produced.

A synthetic response file tests the reporting software. A live run tests the selected models under the recorded task, prompt, and configuration. Neither is a population study. A public holdout demonstrates separation mechanics but cannot establish freedom from model pretraining contamination or independent evaluation.

## What a 99.9% ceiling does and does not mean

The ceiling is a product convention for an official rubric score. It is not an estimated probability that an answer is true, and it does not create a scientifically justified residual error rate of 0.1%. A complete formal proof also does not become a "99.9%-completed proof" because of this display convention.

The [proof achievement](PROOF_GUIDE.md) attaches a playful 10,000% overflow display to a separately checked result. It never enters official grade arithmetic, evidence confidence, or benchmark accuracy. Its message identifies the display as a celebration. An AI-generated statement claiming to contain a proof is insufficient.

The initial Rust checker supports finite propositional logic with at most eight variables. It recomputes every truth assignment for the locked challenge; a certificate with omitted assignments or incorrect truth values cannot pass. The challenge is bound to its grading contract and a known criterion. The achievement also requires fully settled maximum raw current anchors and no blocking grading or evidence state. This supplies a working narrow example of checking an AI-proposed certificate, not a general theorem prover. Translating the natural-language task into the intended formal statement remains the task author's responsibility.

Keeping these outputs separate prevents a new version of the oldest failure: allowing a striking number or an impressive presentation to stand in for the thing actually verified.

## Limits still requiring independent work

The current project does not establish broad educational validity, subgroup fairness, multilingual reliability, or exhaustive coverage of scientific perspectives. Concrete next experiments are:

1. Have subject specialists independently adjudicate criterion expectations, including plausible minority interpretations and appeals.
2. Collect suitable, permissioned answers across tasks and language backgrounds, then inspect error and withholding patterns without treating a group label as an ability score.
3. Freeze genuinely separate tasks for evaluation and retain unsuccessful results. Use repeated fresh judgments to measure variation instead of repeatedly selecting a favorable cached answer.
4. Compare source evidence and applicability, including counterevidence. A URL or matching excerpt does not authenticate a document or establish its truth.
5. Expand formal verification only for precisely specified domains. Publish unsupported cases and checker limits alongside successful demonstrations.

The [evidence workflow](EVIDENCE_GUIDE.md), immutable run archives, and [application interface](API_GUIDE.md) support parts of this work. They do not replace the empirical validation itself.

## Sources

1. Ellis B. Page (1967). *Statistical and Linguistic Strategies in the Computer Grading of Essays*. COLING. [Original paper and metadata](https://aclanthology.org/C67-1032/). Sections on the project's start, intrinsic variables versus proxies, and regression prediction.
2. Donald E. Powers, Jill Burstein, Martin Chodorow, Mary E. Fowles, and Karen Kukich (2001). *Stumping E-Rater: Challenging the Validity of Automated Essay Scoring*. ETS Research Report RR-01-03. [ETS record and abstract](https://www.ets.org/research/policy_research_reports/publications/report/2001/hygl.html). The public abstract supports the limited sole-scorer claim here; this review does not reconstruct inaccessible experiment details.
3. Martin Chodorow and Jill Burstein (2004). *Beyond Essay Length: Evaluating e-rater's Performance on TOEFL Essays*. ETS Research Report RR-04-04. [ETS record and abstract](https://www.ets.org/research/policy_research_reports/publications/report/2004/hshw.html). The improvement from e-rater99 to e-rater01 is part of the finding, not an exception to omit.
4. Anubha Kabra, Mehar Bhatia, Yaman Kumar, Junyi Jessy Li, and Rajiv Ratn Shah (2020; revised 2021). *Evaluation Toolkit For Robustness Testing Of Automatic Essay Scoring Systems*. [Authors' arXiv manuscript, version 5](https://arxiv.org/abs/2007.06796v5). Results concern the five studied scorers and perturbations, not all possible systems.
5. Yuning Ding, Brian Riordan, Andrea Horbach, Aoife Cahill, and Torsten Zesch (2020). *Don't take "nswvtnvakgxpm" for an answer: The surprising vulnerability of automatic content scoring systems to adversarial input*. COLING, pp. 882-892. [Original paper](https://aclanthology.org/2020.coling-main.76/). The paper also reports that adversarial training improves robustness without fully solving the problem.
6. Masaki Uto and Masashi Okano (2020). *Robust Neural Automated Essay Scoring Using Item Response Theory*. AIED, pp. 549-561. [Original research, open full text](https://pmc.ncbi.nlm.nih.gov/articles/PMC7334153/). This is evidence about rater effects in training data, not a claim that every human rating is biased in the same direction.
7. Benjamin Simon Clegg, Phil McMinn, and Gordon Fraser (2021). *An Empirical Study to Determine if Mutants Can Effectively Simulate Students' Programming Mistakes to Increase Tutors' Confidence in Autograding*. SIGCSE. [Author-hosted paper](https://philmcminn.com/publications/clegg2021a.pdf), [DOI](https://doi.org/10.1145/3408877.3432411). Mutation and coverage are test-quality indicators, not universal proofs of correctness.
8. Xiao Liu, Shuai Wang, Pei Wang, and Dinghao Wu (2019). *Automatic Grading of Programming Assignments: An Approach Based on Formal Semantics*. ICSE-SEET, pp. 126-137. [Original paper copy](https://www.cs.tufts.edu/~nr/cs257/archive/autograde-icse-2019.pdf), [DOI](https://doi.org/10.1109/ICSE-SEET.2019.00022). Read its limitations section alongside its experimental results.
