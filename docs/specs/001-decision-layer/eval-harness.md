# Decision-layer eval harness (spec R53)

Goal: compare hosted Jev vs a local open-weight backend on a **labeled sample per
workload** before tuning thresholds (ADR 0001/0002).

## Workloads
| Workload | Question | Labels |
|---|---|---|
| agent routing | `choice` over agent ids + `none` | expected agent id |
| compounder category | `choice` over the 8 categories | expected category |
| outcome | `choice` over done/failed/revised/stalled | expected outcome |

## Method
1. Freeze the question interface (same instructions/criteria across backends).
2. Build a representative labeled set (~100–300 examples per workload).
3. For each backend/`model`, call `/v1/systemone` and record the selected label + confidence.
4. Report **accuracy**, **calibration** (reliability by confidence bin), and **latency**.
5. Sweep `accept_threshold`/`review_threshold` and pick bands whose in-band error is tolerable.

## Usage
Run against hosted and a local backend by setting `DECISION_BASE_URL`/`DECISION_MODEL`
(and `OPENROUTER_API_KEY` for hosted), then writing results to
`docs/specs/001-decision-layer/eval-results.md`.

> Status: harness interface specified; a runnable script is pending (tracked as task T44).
> No backend credentials are stored in the repo.
