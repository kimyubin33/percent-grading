# Synthetic evidence fixtures

These files are manually authored examples for testing a source-review protocol. They are not retrieved scientific sources or actual OpenAI/Claude outputs. The example.org URLs are illustrative identifiers and are never fetched.

- `apple.dossier.json` provides the exact locked task reference and short synthetic explanatory material.
- `openai.review.json` and `claude.review.json` agree on the supplied links under the fixture-only admission policy. Their provider names identify protocol roles, not the origin of the text.
- `claude.disagreement.review.json` disputes the optical-basis link to demonstrate unresolved evidence.
- `apple-missing.dossier.json` omits the reasoning link while retaining all five required claims in the revised evidence policy.
- `openai.missing.review.json` and `claude.missing.review.json` are bound to that revised input.
- `cohort.json` declares three illustrative submissions, including two identical answer hashes under different submission IDs. It is not a roster of real learners.

Use `policies/apple-evidence.aha` for the first dossier and `policies/apple-evidence-v2.aha` for the missing-link revision. Review hashes bind to those exact inputs and `policies/apple.aha`. See the [evidence guide](../../docs/EVIDENCE_GUIDE.md) for executable commands and limits.

The fixtures deliberately permit synthetic material to test the code. A substantive assessment needs a domain-appropriate policy, authentic source excerpts, and real source inspections. Passing this demonstration does not validate the apple explanation scientifically or measure model accuracy.
