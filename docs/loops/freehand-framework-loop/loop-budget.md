# Freehand Framework Loop Budget

- mode: `L1`
- max_wall_time_minutes: 20
- max_shell_commands: 12
- max_write_files: 2
- max_code_edits: 0
- max_attempts_per_item: 3
- max_items_per_run: 1

## Required Early Exit

Exit and log `no-op` when:

- kill switch is active
- no watchlist item is actionable
- budget is exhausted
- owner mapping is unclear
- current workspace state suggests another worker owns the relevant dirty files

