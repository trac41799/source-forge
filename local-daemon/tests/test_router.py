from router import (
    audit_decision,
    build_router_prompt,
    build_router_questions,
    decision_thresholds,
    parse_decision_response,
    parse_router_response,
    policy_action,
)


class TestBuildRouterPrompt:
    def test_includes_agent_list_and_message(self):
        agents = [
            {"id": "assistant", "display_name": "Assistant", "description": "General help"},
            {"id": "code-reviewer", "display_name": "Code Reviewer", "description": "Reviews PRs"},
        ]
        prompt = build_router_prompt(
            platform="lark",
            sender_name="Alice",
            text="Review my PR please",
            agents=agents,
        )
        assert "assistant" in prompt
        assert "code-reviewer" in prompt
        assert "General help" in prompt
        assert "Reviews PRs" in prompt
        assert "Alice" in prompt
        assert "lark" in prompt
        assert "Review my PR please" in prompt

    def test_empty_agents_produces_none_marker(self):
        prompt = build_router_prompt(
            platform="slack",
            sender_name="Bob",
            text="Hello",
            agents=[],
        )
        assert "(none)" in prompt
        assert "Bob" in prompt
        assert "slack" in prompt

    def test_includes_platform_field(self):
        prompt = build_router_prompt(
            platform="discord",
            sender_name="Carol",
            text="ping",
            agents=[{"id": "bot", "display_name": "Bot", "description": "Bot"}],
        )
        assert "discord" in prompt


class TestParseRouterResponse:
    def test_parses_single_agent_id(self):
        result = parse_router_response("assistant")
        assert result == "assistant"

    def test_parses_agent_id_with_extra_whitespace(self):
        result = parse_router_response("  code-reviewer  \n")
        assert result == "code-reviewer"

    def test_parses_agent_id_from_response_with_newline(self):
        result = parse_router_response("assistant\n")
        assert result == "assistant"

    def test_strips_explanation_and_returns_first_word(self):
        result = parse_router_response("assistant explanation text here")
        assert result == "assistant"

    def test_none_returns_none(self):
        result = parse_router_response("none")
        assert result is None

    def test_none_with_whitespace_returns_none(self):
        result = parse_router_response("  none  ")
        assert result is None

    def test_empty_string_returns_none(self):
        result = parse_router_response("")
        assert result is None

    def test_only_punctuation_returns_none(self):
        result = parse_router_response(".")
        assert result is None


class TestDecisionRouting:
    def test_build_router_questions_offers_agents_and_none(self):
        agents = [{"id": "a", "description": "A"}, {"id": "b", "display_name": "B"}]
        q = build_router_questions(agents)["agent"]
        assert q["type"] == "choice"
        assert set(q["criteria"]) == {"a", "b", "none"}

    def test_build_router_questions_skips_empty_ids(self):
        agents = [{"id": "", "description": "x"}, {"id": "b", "description": "B"}]
        q = build_router_questions(agents)["agent"]
        assert set(q["criteria"]) == {"b", "none"}

    def test_parse_decision_choice(self):
        payload = {"answers": {"agent": {"type": "choice", "choice": "b", "confidence": 0.91}}}
        assert parse_decision_response(payload) == ("b", 0.91)

    def test_parse_decision_none_option(self):
        payload = {"answers": {"agent": {"type": "choice", "choice": "none", "confidence": 0.4}}}
        assert parse_decision_response(payload) == (None, 0.4)

    def test_parse_decision_malformed_returns_none(self):
        assert parse_decision_response({}) is None
        assert parse_decision_response({"answers": {}}) is None
        assert (
            parse_decision_response({"answers": {"agent": {"type": "noul", "noul": 0.5}}})
            is None
        )


class TestPolicyActionBand:
    """R-3: the daemon must apply the same accept/review band as the app."""

    def test_apply_at_or_above_accept(self):
        assert policy_action(0.80, 0.75, 0.40) == "apply"
        assert policy_action(0.75, 0.75, 0.40) == "apply"

    def test_review_between_thresholds(self):
        assert policy_action(0.74, 0.75, 0.40) == "review"
        assert policy_action(0.40, 0.75, 0.40) == "review"

    def test_skip_below_review(self):
        assert policy_action(0.39, 0.75, 0.40) == "skip"


class TestDecisionThresholds:
    def test_defaults_match_app(self, monkeypatch):
        monkeypatch.delenv("DECISION_ACCEPT_THRESHOLD", raising=False)
        monkeypatch.delenv("DECISION_REVIEW_THRESHOLD", raising=False)
        assert decision_thresholds() == (0.75, 0.40)

    def test_env_override(self, monkeypatch):
        monkeypatch.setenv("DECISION_ACCEPT_THRESHOLD", "0.9")
        monkeypatch.setenv("DECISION_REVIEW_THRESHOLD", "0.5")
        assert decision_thresholds() == (0.9, 0.5)

    def test_bad_env_falls_back_to_default(self, monkeypatch):
        monkeypatch.setenv("DECISION_ACCEPT_THRESHOLD", "not-a-number")
        assert decision_thresholds()[0] == 0.75


class TestAuditDecision:
    def test_emits_json_usage_line(self, caplog):
        import json as _json
        import logging as _logging

        with caplog.at_level(_logging.INFO):
            rec = audit_decision(
                "daemon.router", "http://x", "m", "b", 0.91, "apply", 12.5
            )
        assert rec["policy_outcome"] == "apply"
        assert rec["chosen"] == "b"
        joined = "\n".join(r.getMessage() for r in caplog.records)
        assert "decision_usage" in joined
        payload = joined.split("decision_usage ", 1)[1]
        assert _json.loads(payload)["confidence"] == 0.91


def _install_fake_httpx(monkeypatch, payload):
    """Inject a fake `httpx` module so route_with_decision needs no real HTTP."""
    import sys
    import types

    calls = {"n": 0, "body": None}

    class _Resp:
        status_code = 200

        def raise_for_status(self):
            return None

        def json(self):
            return payload

    class _Client:
        def __init__(self, **kw):
            pass

        async def __aenter__(self):
            return self

        async def __aexit__(self, *args):
            return False

        async def post(self, url, json=None, headers=None):
            calls["n"] += 1
            calls["body"] = json
            return _Resp()

    fake = types.ModuleType("httpx")
    fake.AsyncClient = _Client
    monkeypatch.setitem(sys.modules, "httpx", fake)
    return calls


class TestRouteWithDecisionBand:
    """L4: the band decision path in route_with_decision must be exercised."""

    @staticmethod
    def _ctx():
        from types import SimpleNamespace

        return SimpleNamespace(text="hi", sender_name="a", platform="lark")

    @staticmethod
    def _project():
        return {"agents": [{"id": "b", "description": "B"}]}

    def _run(self, monkeypatch, payload):
        import asyncio

        from router import route_with_decision

        monkeypatch.setenv("DECISION_BASE_URL", "http://127.0.0.1:8009")
        monkeypatch.delenv("OPENROUTER_API_KEY", raising=False)
        monkeypatch.delenv("DECISION_ACCEPT_THRESHOLD", raising=False)
        monkeypatch.delenv("DECISION_REVIEW_THRESHOLD", raising=False)
        calls = _install_fake_httpx(monkeypatch, payload)
        result = asyncio.run(route_with_decision(self._ctx(), self._project()))
        return result, calls

    def test_apply_band_uses_decision(self, monkeypatch):
        result, calls = self._run(
            monkeypatch,
            {"answers": {"agent": {"type": "choice", "choice": "b", "confidence": 0.9}}},
        )
        assert result == ("b", 0.9)
        assert calls["n"] == 1, "exactly one decision request"

    def test_review_band_falls_back(self, monkeypatch):
        result, _ = self._run(
            monkeypatch,
            {"answers": {"agent": {"type": "choice", "choice": "b", "confidence": 0.5}}},
        )
        assert result is None, "review band must fall back to the prompt router"

    def test_skip_band_falls_back(self, monkeypatch):
        result, _ = self._run(
            monkeypatch,
            {"answers": {"agent": {"type": "choice", "choice": "b", "confidence": 0.2}}},
        )
        assert result is None

    def test_none_choice_returns_none_agent(self, monkeypatch):
        result, _ = self._run(
            monkeypatch,
            {"answers": {"agent": {"type": "choice", "choice": "none", "confidence": 0.9}}},
        )
        assert result == (None, 0.9), "explicit 'none' is preserved (not a fallback)"

    def test_malformed_response_returns_none(self, monkeypatch):
        result, _ = self._run(monkeypatch, {})
        assert result is None
