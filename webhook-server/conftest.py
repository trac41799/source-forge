"""Pytest path setup for webhook-server.

The project directory name contains a hyphen, so it cannot be imported as
`webhook_server` directly. Register an alias package pointing at this directory
so tests can use `from webhook_server...` imports, and put this directory on
sys.path for top-level modules (adapters, settings, models, message_queue, ...).
"""

import pathlib
import sys
import types

_HERE = pathlib.Path(__file__).resolve().parent

sys.path.insert(0, str(_HERE))

if "webhook_server" not in sys.modules:
    _pkg = types.ModuleType("webhook_server")
    _pkg.__path__ = [str(_HERE)]
    sys.modules["webhook_server"] = _pkg
