#!/usr/bin/env python3
"""Decision-layer eval harness (spec 001, R53).

Compares decision backends on a labeled sample: accuracy, mean latency, and
confidence. No credentials are stored here — read from the environment.

Dataset: one JSON object per line:
    {"state": "...", "question": {"q": {"type": "choice", "instructions": "...", "criteria": {...}}}, "expected": "label"}

Usage:
    python evaluate.py --dataset data.jsonl
    DECISION_BASE_URL=http://127.0.0.1:8009 DECISION_MODEL=von-1.0 python evaluate.py --dataset data.jsonl
"""
import argparse
import json
import os
import statistics
import sys
import time
import urllib.request


def call(base_url, model, key, state, questions):
    body = json.dumps({"model": model, "state": state, "questions": questions}).encode()
    headers = {"Content-Type": "application/json"}
    if key:
        headers["Authorization"] = f"Bearer {key}"
    req = urllib.request.Request(
        f"{base_url.rstrip('/')}/v1/systemone", data=body, headers=headers
    )
    with urllib.request.urlopen(req, timeout=10) as resp:
        return json.loads(resp.read())


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dataset", required=True)
    ap.add_argument("--base-url", default=os.environ.get("DECISION_BASE_URL", "https://openrouter.ai/api"))
    ap.add_argument("--model", default=os.environ.get("DECISION_MODEL", "typesafe/jev-1.13"))
    ap.add_argument("--question-id", default="q")
    args = ap.parse_args()

    key = os.environ.get("OPENROUTER_API_KEY", "")
    correct = 0
    total = 0
    latencies = []
    confidences = []

    for line in open(args.dataset, encoding="utf-8"):
        line = line.strip()
        if not line:
            continue
        row = json.loads(line)
        t0 = time.perf_counter()
        try:
            resp = call(args.base_url, args.model, key, row["state"], row["question"])
        except Exception as e:  # noqa: BLE001 - harness reports and continues
            print(f"error: {e}", file=sys.stderr)
            continue
        latencies.append((time.perf_counter() - t0) * 1000.0)

        ans = (resp.get("answers") or {}).get(args.question_id) or {}
        got = ans.get("choice")
        if got is None:
            got = ans.get("noul")
        if ans.get("confidence") is not None:
            confidences.append(float(ans["confidence"]))

        total += 1
        if row.get("expected") is not None and got == row["expected"]:
            correct += 1

    acc = (correct / total) if total else 0.0
    mean_lat = statistics.mean(latencies) if latencies else 0.0
    mean_conf = statistics.mean(confidences) if confidences else 0.0
    print(
        f"backend={args.base_url} model={args.model} n={total} "
        f"accuracy={acc:.3f} mean_latency_ms={mean_lat:.0f} mean_confidence={mean_conf:.3f}"
    )


if __name__ == "__main__":
    main()
