# Decision-layer eval harness

Runnable harness for spec 001 R53 (compare decision backends on a labeled sample).

See `../../docs/specs/001-decision-layer/eval-harness.md` for method.

## Run
```bash
# hosted (default)
OPENROUTER_API_KEY=... python evaluate.py --dataset sample.jsonl

# local backend
DECISION_BASE_URL=http://127.0.0.1:8009 DECISION_MODEL=von-1.0 python evaluate.py --dataset sample.jsonl
```

Output: `n`, `accuracy`, `mean_latency_ms`, `mean_confidence`.

## Dataset
One JSON object per line:
```json
{"state": "Fix the failing login test", "question": {"q": {"type": "choice", "instructions": "Which category?", "criteria": {"pattern": "", "antipattern": ""}}}, "expected": "pattern"}
```
