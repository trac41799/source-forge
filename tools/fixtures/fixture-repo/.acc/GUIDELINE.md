# AGENT probe GUIDELINE

## Objective
Produce a schema-valid handoff file proving the agent CLI can execute tools
headlessly.

## Task
1. Create `probe-output.txt` containing the single line `probe ok`.
2. Create a handoff file named exactly `HANDOFF_probe.md` containing exactly
   these six markdown section headers, each with at least one line of content:

   ```
   ## Completed Work
   ## Test Results
   ## Interface Contracts Exposed
   ## Files NOT Modified
   ## Design Decisions
   ## Handoff Instructions
   ```

All six headers are required verbatim. Do not omit any.
