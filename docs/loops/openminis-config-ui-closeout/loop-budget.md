# OpenMinis Config UI Closeout Budget

- mode: `L2 assisted`
- max_wall_time_minutes: 120
- max_shell_commands: 80
- max_write_files: 18
- max_code_edits: 1 scoped batch
- max_attempts_per_item: 3
- max_items_per_run: 1

## L1 Exit Criteria

Exit after:

- OpenMinis reference evidence is identified.
- Freehand config/UI gaps are mapped to owners.
- L2 batch plan and required gates are written.
- Run log records report-only outcome.

## L2 Batch Criteria

Exit after one scoped batch:

- owner maps and test designs are updated before/with code
- implementation touches only the approved owner chain for the batch
- positive and negative tests cover the new projection path
- S-profile online WebUI proof on `127.0.0.1:4042` passes
- run log records commands and evidence

## Required Early Exit

Exit and report if:

- kill switch is active
- OpenMinis reference cannot be reached and no local reference exists
- owner mapping is unclear
- dirty workspace appears to contain another worker's active changes in the same target files
