import json
import logging
import os
import re
from typing import Optional

from adapters import ToolResult
from adapters.coding_tool import CodingToolAdapter

logger = logging.getLogger(__name__)

ROUTER_SYSTEM_PROMPT = """\
You are a message router. Given a list of available agents and a user message,
output ONLY the id of the agent that should handle the message.
Output a single word: either an agent id, or "none" if no agent matches.
Do not output any other text, explanation, or punctuation."""

ROUTER_PROMPT_TEMPLATE = """\
Available agents:
{agent_list}

Message from {sender_name} on {platform}:
{text}

Which agent should handle this message? Output only the agent id or "none":"""


def build_router_prompt(
    platform: str,
    sender_name: str,
    text: str,
    agents: list[dict],
) -> str:
    agent_lines = []
    for agent in agents:
        agent_id = agent.get("id", "")
        display_name = agent.get("display_name", agent_id)
        description = agent.get("description", "")
        agent_lines.append(f"  - {agent_id}: {display_name} — {description}")

    agent_list = "\n".join(agent_lines) if agent_lines else "  (none)"

    return ROUTER_PROMPT_TEMPLATE.format(
        agent_list=agent_list,
        sender_name=sender_name,
        platform=platform,
        text=text,
    )


def parse_router_response(response: str) -> Optional[str]:
    for line in response.strip().splitlines():
        line = line.strip()
        if not line:
            continue
        for word in line.split():
            cleaned = re.sub(r"[^a-z0-9_\-]", "", word.lower())
            if cleaned:
                if cleaned == "none":
                    return None
                return cleaned
    return None


# ── Decision layer (spec R10) ─────────────────────────────────────────────
DECISION_BASE_DEFAULT = "https://openrouter.ai/api"
DECISION_MODEL_DEFAULT = "typesafe/jev-1.13"


def build_router_questions(agents: list[dict]) -> dict:
    """A `choice` question over the configured agent ids plus a `none` option."""
    criteria: dict = {}
    for agent in agents:
        agent_id = agent.get("id", "")
        if not agent_id:
            continue
        criteria[agent_id] = (
            agent.get("description") or agent.get("display_name") or agent_id
        )
    criteria["none"] = "No available agent should handle this message"
    return {
        "agent": {
            "type": "choice",
            "instructions": "Which agent should handle this message?",
            "criteria": criteria,
        }
    }


def parse_decision_response(payload: dict):
    """Return (agent_id_or_None, confidence) from a /v1/systemone response.

    Returns None when the response is malformed/absent (caller falls back).
    A tuple with agent_id None means the model chose "none".
    """
    answers = payload.get("answers") or {}
    ans = answers.get("agent") or {}
    if ans.get("type") != "choice":
        return None
    choice = ans.get("choice")
    if not choice:
        return None
    confidence = float(ans.get("confidence", 0.0))
    return (None if choice == "none" else choice, confidence)


async def route_with_decision(payload, project: dict):
    """Route via the decision endpoint. Returns (agent_id|None, confidence) or None on failure."""
    import httpx  # local import: tests need not have httpx installed

    agents = project.get("agents", [])
    if not agents:
        return None

    base = os.environ.get("DECISION_BASE_URL", DECISION_BASE_DEFAULT).rstrip("/")
    key = os.environ.get("OPENROUTER_API_KEY", "")
    is_local = "127.0.0.1" in base or "localhost" in base
    if not key and not is_local:
        return None

    body = {
        "model": os.environ.get("DECISION_MODEL", DECISION_MODEL_DEFAULT),
        "state": {
            "message": payload.text,
            "sender": payload.sender_name,
            "platform": payload.platform,
        },
        "questions": build_router_questions(agents),
    }
    headers = {"Content-Type": "application/json"}
    if key:
        headers["Authorization"] = f"Bearer {key}"
    try:
        async with httpx.AsyncClient(timeout=5.0) as client:
            resp = await client.post(f"{base}/v1/systemone", json=body, headers=headers)
            resp.raise_for_status()
            return parse_decision_response(resp.json())
    except Exception as e:
        logger.error("Decision router failed: %s", e)
        return None


async def route(
    payload,  # StandardContextPayload
    project: dict,
    adapter: CodingToolAdapter,
    cwd: str = ".",
) -> Optional[str]:
    agents = project.get("agents", [])
    if not agents:
        return None

    # M1 (spec R10): decide via the decision layer; fall back to prompt+parse.
    decision = await route_with_decision(payload, project)
    if decision is not None:
        agent_id, confidence = decision
        logger.info("Router (decision) → agent_id=%s conf=%.2f", agent_id, confidence)
        return agent_id

    prompt = build_router_prompt(
        platform=payload.platform,
        sender_name=payload.sender_name,
        text=payload.text,
        agents=agents,
    )

    try:
        result: ToolResult = await adapter.run(
            prompt=ROUTER_SYSTEM_PROMPT + "\n\n" + prompt,
            cwd=cwd,
            timeout=30,
        )
    except Exception as e:
        logger.error("Router adapter failed: %s", e)
        return None

    if result.returncode != 0:
        logger.error("Router returned non-zero: %d, stderr: %s", result.returncode, result.stderr)
        return None

    agent_id = parse_router_response(result.stdout)
    logger.info("Router classified message → agent_id=%s", agent_id)
    return agent_id
