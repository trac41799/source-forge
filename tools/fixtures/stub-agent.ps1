param([string]$Prompt = "")

# Deterministic stub agent for the Wave D E2E probe.
# Ignores the prompt, waits briefly, then writes a schema-valid handoff.

Start-Sleep -Milliseconds 400

$handoff = @"
# HANDOFF stub

## Original Task
$Prompt

## Completed By
stub-agent

## Model Used
stub-1

## Output Summary
Stub agent completed the probe task.

## Completed Work
Wrote a valid handoff for the probe.

## Test Results
- probe: 1 passed, 0 failed

## Files Changed
- (none)

## Files NOT Modified
- (all)

## Design Decisions
None.

## Interface Contracts Exposed
None.

## Handoff Instructions
None.
"@

Set-Content -LiteralPath (Join-Path (Get-Location) "HANDOFF_stub.md") -Value $handoff -Encoding UTF8
Write-Output "[stub-agent] handoff written"
