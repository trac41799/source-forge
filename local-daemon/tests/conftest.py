import os
import sys
import types
from pathlib import Path
from unittest.mock import AsyncMock, MagicMock

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
sys.path.insert(0, str(Path(__file__).resolve().parent.parent.parent))

# The webhook-server directory name contains a hyphen, so it cannot be imported
# as `webhook_server` directly. Alias it as an importable package for tests.
_REPO_ROOT = Path(__file__).resolve().parent.parent.parent
if "webhook_server" not in sys.modules:
    _pkg = types.ModuleType("webhook_server")
    _pkg.__path__ = [str(_REPO_ROOT / "webhook-server")]
    sys.modules["webhook_server"] = _pkg


@pytest.fixture
def sample_registry_yaml():
    return """
platforms:
  - routing_key: "lark:cli_abc123"
    platform: "lark"
    root: "/home/user/my-project/lark-agents"
    router_adapter: "claude"
    agent_adapter: "claude"
    reply_mode: "post_process"
    agents:
      - id: assistant
        display_name: "Assistant"
        description: "General-purpose assistant"
      - id: code-reviewer
        display_name: "Code Reviewer"
        description: "Reviews pull requests and suggests fixes"
  - routing_key: "slack:T789012"
    platform: "slack"
    root: "/home/user/slack-project/agents"
    router_adapter: "opencode"
    agent_adapter: "opencode"
    reply_mode: "inline"
    agents:
      - id: support-bot
        display_name: "Support Bot"
        description: "Handles customer support questions"
"""


@pytest.fixture
def sample_payload():
    from webhook_server.models import StandardContextPayload
    return StandardContextPayload(
        platform="lark",
        routing_key="lark:cli_abc123",
        chat_id="oc_abc123def456",
        chat_type="group",
        channel_name="Engineering",
        message_id="msg_001",
        sender_id="user_001",
        sender_name="Alice",
        text="Can you review my latest PR?",
        ts="2026-05-05T10:30:00Z",
    )


@pytest.fixture
def sample_project():
    return {
        "routing_key": "lark:cli_abc123",
        "platform": "lark",
        "root": "/home/user/my-project/lark-agents",
        "router_adapter": "claude",
        "agent_adapter": "claude",
        "reply_mode": "post_process",
        "agents": [
            {"id": "assistant", "display_name": "Assistant", "description": "General-purpose assistant"},
            {"id": "code-reviewer", "display_name": "Code Reviewer", "description": "Reviews pull requests and suggests fixes"},
        ],
    }
