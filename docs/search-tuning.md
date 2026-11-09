# How we tune search

Clay UI is only useful if it finds the right component. This is how we measure that, what we changed, and how we keep improving it.

## How we measure

We built a benchmark before changing anything, then froze it:

- **Synthetic development requests.** 120 distinct needs, each written in English and Polish. Translations are the same intent, not independent examples. A separate sealed final set is reserved for the owner. They include short casual asks, descriptions that never name the component, several requirements at once, typos, and project context.
- **Requests that should find nothing.** A good search also knows when no component fits.
- **Independent judging.** Two different models graded each result from 0 to 3 without seeing its rank or which method found it. A third settled disagreements. Their agreement was strong (weighted kappa 0.90).
- **A strict definition of success.** A result counts only with a top grade: a ready base that meets the essential requirements. Something merely related does not count.

The sealed set is never used for tuning. It is measured once, on the finished code, so the number stays honest.

## What we measure

| Metric | Question it answers |
| --- | --- |
| Success@5 | Is a ready base in the top 5? |
| Context success@5 | Is it still right for this particular project? |
| Correct rejection | Does it stay empty when nothing fits? |

## Earlier measurements

Development set, English queries, conservative scoring. Results that were never graded count as misses.

| Setup | Success@5 | Context success@5 | Correct rejection |
| --- | ---: | ---: | ---: |
| Starting point, Gemini key | 59.0% | not comparable | not comparable |
| Tuned, Gemini key | 65.7% | 48.3% | 20.0% |
| Tuned, no key | 41.9% | 33.3% | 33.3% |

Our target is 95% with a Gemini key, and we are not there yet. These are historical Node retrieval results with simulated concise queries and source routing. They do not measure the Rust binary or an executed agent loop. In real use a coding agent reads sources and checks the project; that boundary must be measured separately.

## Final measurement for 0.1.0

We ran the sealed set once, on the finished search code. Only English queries were scored, and results nobody graded count as misses. The "best case" column counts every ungraded result as perfect.

| Mode | Success@5 | Best case | Context success@5 | Correct rejection |
| --- | ---: | ---: | ---: | ---: |
| Gemini key | 56.2% | 87.6% | 20.0% | 53.3% |
| No key | 39.0% | 47.6% | 12.0% | 73.3% |

We did not reach our 95% target. A verified match was in the top 5 for 59 of 105 needs with a key and 41 of 105 without one. These numbers cover the CLI alone. In real use the agent also reads the code and reshapes it, and all five full end-to-end acceptance runs ended with a working component.

## What helped

- **Better queries from the agent.** Component type first, then the behavior that matters. This was the largest gain: 32 percentage points without a key.
- **A complete catalog.** Some libraries were missing whenever GitHub rate-limited the sync.

## What did not help

- Richer text for the embeddings, and fusing keyword and semantic rankings: no gain beyond measurement noise.
- A higher similarity threshold: it rejected more, and lost too many good results.
- A separate re-ranking step: nothing justified the added complexity.
- Penalties for a missing demo or maintenance note: they say nothing about whether a component fits.

## What limits us now

For about a third of the benchmark's needs, no catalog component was confirmed as a ready base. Ranking cannot fix that. These need source recovery and complete examples from existing sources first. New sources need evidence of a remaining gap.

## Rules for future tuning

1. Change one thing and measure it on the development set.
2. Keep a change only if the gain is larger than the measurement noise.
3. Prefer better agent instructions or a better catalog to more search machinery.
4. Never look at the sealed set while tuning.
5. Publish the number you actually measured.


## November 2026 experiments

We ran 96 fresh agent contexts: one English need from each of 24 families, one repetition, the current skill versus an explicit bounded verification policy, with and without Gemini. Each context used the actual Rust CLI and inspected source. Both policies in both modes ended with an independently verified good example for 16 of 21 positive needs (76.2%; family bootstrap intervals about 57–95%). All arms rejected the three out-of-scope needs. That is too small a sample to certify rejection or precision at 95%.

The bounded policy did not improve the final choice. It increased ready recommendations with Gemini, but reduced them without a key. We kept the current workflow, including the intent-reading section.

On the full English development set, the same fixed Rust ranker with concise simulated queries gave these conservative results after both sides were rescored on the same new independent evidence:

| Representation / catalog | Gemini Success@5 | No-key Success@5 |
| --- | ---: | ---: |
| Existing catalog, original text | 69.5% | 62.9% |
| Source cards, disputed assertions removed | 68.6% | 65.7% |
| Existing catalog plus downloaded complete examples | 75.2% | 68.6% |

The cards did not pass their claim audit or establish a ranking gain beyond noise. The example expansion gained 5.7 percentage points in each mode, with a paired family interval of 0–15.2 points. Gains concentrated in two families. Neither change passed the adoption rule, so neither became the default index. These are retrieval-function experiments, separate from the 24-need full agent pilot above.

Restored source evidence and complete examples now confirm a ready base for 83 of 105 positive development needs. That is a known-evidence coverage limit, not proof that the other needs have no solution. Unknown judgments remain unknown. Dev is still not a final-set certificate, and 95% has not been demonstrated.

The optional static catalog transport is prepared with an empty `DEFAULT_CDN_BASE_URL` and an explicit `CLAYUI_CDN_BASE_URL` override. It validates untrusted HTTPS data and falls back to current sources. Delivery support is not a measured relevance improvement; no experimental catalog was uploaded or enabled by default.
