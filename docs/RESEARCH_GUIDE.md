# Research discovery and perspective selection

Version 0.3.0 adds two distinct operations: bibliographic discovery through Crossref and deterministic selection from a declared perspective catalog. Neither operation changes the grading contract's weights, anchors, or accepted conclusions. OpenAI and Claude remain the only AI judges.

## 1. What is automatic

`discover` sends a bounded search query to the official Crossref metadata API and saves bibliographic candidates. It does not download papers or treat their titles as established scientific findings.

`select` examines a local catalog against the checked task and automatically chooses relevant entries under count, overlap, and context-size limits. It reads the question, context, reference, and criterion descriptions. It does **not** read the learner's answer. This avoids selecting a convenient theory after seeing the conclusion that needs defending.

The bundled [apple catalog](../catalogs/apple.json) contains eight declared perspectives, including a deliberate overlap pair and an irrelevant control. Supporting excerpts are short curator paraphrases. It is a teaching catalog, not a collection of thousands of independently validated theories.

The implementation accepts catalogs with up to 10,000 perspectives and 10,000 sources within its byte limit. That is an input limit tested in software, not evidence of accuracy over 10,000 theories. Only a bounded selection is sent to the AI judges.

## 2. Run the offline selection first

From the repository root:

```text
cargo run --locked -- compile policies/apple.aha --out local-runs/apple.lock.json
cargo run --locked -- select local-runs/apple.lock.json catalogs/apple.json --out local-runs/apple.research.json
cargo run --locked -- inspect-research local-runs/apple.lock.json local-runs/apple.research.json
```

If the contract already exists, reuse it instead of repeating the first command. Output files are created without overwriting existing files. Choose another output path when intentionally creating a new snapshot.

For a smaller selection:

```text
cargo run --locked -- select local-runs/apple.lock.json catalogs/apple.json --out local-runs/apple.small.research.json --max-perspectives 3 --max-context-bytes 8000
```

Inspect selected perspective IDs, matched keywords, mapped criterion IDs, source excerpts, excluded candidates, and uncovered criteria. A low keyword match does not prove irrelevance; a high match does not prove applicability. The report exposes the heuristic so it can be reviewed.

The two `reflectance-*` entries share the `surface-spectrum` overlap group. At most one should be selected. `orbital-motion` has no relevant keyword match for the apple task. The future hypothesis maps only to the future criterion.

## 3. Discover bibliographic candidates

This command makes a network request but does not call an AI provider:

```text
cargo run --locked -- discover local-runs/apple.lock.json --query "surface reflectance illuminant color perception" --rows 10 --out local-runs/apple.search.json
```

Without `--query`, the program derives a query from the task. Review the query before using discovery with private task material; the query is transmitted to Crossref. Only the fixed Crossref endpoint is used. The command does not follow arbitrary candidate links, send grading API keys, or add a third AI provider.

The result records titles, DOI identifiers, publishers, publication years when available, and canonical DOI links. A valid DOI is an identifier, not a correctness certificate. Search rank is bibliographic relevance, not scientific support for an answer. The saved bundle has an integrity hash so later changes are detectable under ordinary use.

A search bundle is not automatically promoted into a grading catalog. A curator must inspect the source, determine what it actually supports, write a limited paraphrase or permitted quotation, identify assumptions, and classify the perspective. This boundary prevents a paper title or an attractive abstract fragment from silently becoming a scoring rule.

## 4. Build a catalog with accountable sources

A catalog has `schema_version`, `version`, `sources`, and `perspectives`. Use `percent-grading-catalog-v1` as the schema version. Unknown fields are rejected.

| Record | Fields | Meaning |
|---|---|---|
| Source | `id`, `title`, `url`, `excerpt` | A declared source and the exact limited text supplied as context |
| Perspective | `id`, `name`, `kind`, `description` | Stable identity and what this lens asks the evaluator to inspect |
| Applicability | `assumptions`, `keywords`, `dimensions` | Scope, retrieval hints, and eligible grading dimensions |
| Provenance and overlap | `source_ids`, `overlap` | Source references and the group used to suppress duplicate perspectives |

Use `established` for a perspective grounded in a cited established account, `method` for a declared evaluation procedure, and `hypothesis` for an explicitly hypothetical possibility. This label is the curator's declaration; the program cannot certify it. Hypothesis entries must use only the `future` dimension.

Allowed dimensions are `intent`, `principle`, `reasoning`, `conditions`, and `future`. The selector maps these dimensions to existing criterion IDs. Catalog entries cannot create new scoring criteria or edit their weights. A source reference must resolve to a source in the catalog. IDs and lists must satisfy the program's uniqueness and size checks.

An excerpt is not necessarily a verbatim quotation. The example explicitly labels its source text as a curator paraphrase. Preserve that distinction in new catalogs. The software validates the record and link structure, not whether a paraphrase faithfully represents its source. It does not fetch source URLs during local selection.

## 5. How selection remains bounded

The selector constructs its search material from the fixed contract. It normalizes text, matches distinct whole-word keywords or phrases, and ranks eligible perspectives by match count. Ties use ASCII identifier order. It does not stem words, translate terms, or infer semantic similarity. A selected overlap group occupies only one slot; exact duplicate content is also suppressed. Count and serialized context-byte limits bound the material sent to the judges; catalog size is not the same as prompt size. A candidate that cannot fit does not reserve its overlap group, so a later smaller candidate from that group can still fit.

The default limits are eight perspectives and 16,000 bytes of selected context. The count ceiling is 32 and the configurable context range is 512–65,536 bytes. Exclusions and coverage diagnostics remain visible. A budget can leave a criterion without a selected external perspective. That does not delete the criterion or its original contract guidance.

With the supplied apple policy and catalog, the default selector chooses six entries, excludes `orbital-motion` for no keyword match and `reflectance-b` for overlap, and covers all five criteria. These are reproducible software-selection results, not measurements of model accuracy.

Overlap groups are declared by the curator. The selector does not understand semantic equivalence well enough to discover every duplicate automatically. Renaming the same idea into a new overlap group can therefore evade deduplication. Regardless of the number of selected entries, criterion weights and anchor values remain fixed.

More perspectives can alter AI interpretation and can make it worse. Fixed weights prevent extra numerical credit merely for adding entries; they do not prove that extra context is harmless. Compare judgments before and after a catalog change and preserve both run identities.

## 6. Use the frozen selection in grading

Preview the same selected context that will be available to the judges:

```text
cargo run --locked -- prompt local-runs/apple.lock.json examples/rust/apple-blue.txt --research local-runs/apple.research.json
```

With API credentials configured as described in the [Rust guide](RUST_GUIDE.md):

```text
cargo run --locked -- grade local-runs/apple.lock.json examples/rust/apple-blue.txt --research local-runs/apple.research.json --openai-model YOUR_OPENAI_MODEL --claude-model YOUR_CLAUDE_MODEL
```

The pack retains its catalog snapshot, selection configuration, checked contract identity, selected records, and diagnostics. Verification repeats selection and checks the stored identity. A pack created for another contract is rejected. Editing a saved source, selection, or budget does not silently produce a valid replay.

Both judges receive the same selected material as untrusted reference context. It cannot override the fixed task, add points, or convert a future hypothesis into a present fact. The runtime keeps the pack with the run and includes its identity in the run's content address. Changing the catalog or selection configuration produces a different run identity even when the answer is unchanged. Ordinary `replay` uses the recorded snapshot without contacting Crossref or either AI provider.

The pack is an integrity-checked local record, not a signed attestation from a scientific publisher. A person who controls all local files can replace the inputs and their hashes. Its purpose is transparent reconstruction, not proof of external authorship.

## 7. Source notes for the example

These primary educational sources were checked while preparing the example. The catalog stores concise paraphrases, not complete pages:

- [NASA JPL: The Science of Color](https://www.jpl.nasa.gov/edu/resources/lesson-plan/the-science-of-color/) supports the reflection, absorption, and filter examples.
- [Institute of Physics: Colours of two kinds](https://spark.iop.org/collections/colours-two-kinds-teaching-and-learning-issues) explains why illumination and surface response must be considered together.
- [NASA: Spectroscopy 101 - Light and Matter](https://science.nasa.gov/mission/webb/science-overview/science-explainers/spectroscopy-101-light-and-matter/) supports the limited visible-range discussion.
- [NASA: Orbits and Kepler's Laws](https://science.nasa.gov/solar-system/orbits-and-keplers-laws/) supplies the deliberately irrelevant orbital example.
- [Project Rust guide](RUST_GUIDE.md) supplies the explicitly project-proposed evaluation methods.

The network adapter follows [Crossref's REST API documentation](https://www.crossref.org/documentation/retrieve-metadata/rest-api/). Crossref metadata helps locate publications; it does not validate the claims assigned to catalog entries.

## 8. Experiments to run next

Compare the same fixed task and model configuration with zero external perspectives, a small selection, and a larger selection. Add duplicate and irrelevant records deliberately. Record selection diagnostics, prompt size, model disagreement, false acceptance of unsupported conclusions, and cost.

Do not interpret a successful software test or a faster search as better grading. The empirical question is whether the selected material improves correct acceptance and useful feedback without hiding uncertainty or increasing false confidence.
