# OpenMinis Config UI Closeout Budget

- mode: `L1`
- max_wall_time_minutes: 45
- max_shell_commands: 25
- max_write_files: 7
- max_code_edits: 0
- max_attempts_per_item: 3
- max_items_per_run: 1

## L1 Exit Criteria

Exit after:

- OpenMinis reference evidence is identified.
- Freehand config/UI gaps are mapped to owners.
- L2 batch plan and required gates are written.
- Run log records report-only outcome.

## Required Early Exit

Exit and report if:

- kill switch is active
- OpenMinis reference cannot be reached and no local reference exists
- owner mapping is unclear
- dirty workspace appears to contain another worker's active changes in the same target files
