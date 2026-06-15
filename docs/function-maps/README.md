# Function Maps

This directory is the durable truth for code-bound feature mainlines.

`docs/architecture/feature-map.md` answers:

- who owns the feature
- where it may change
- how it is validated

This directory answers:

- which symbols implement the feature
- how request mainline runs
- how response mainline runs
- how error mainline runs
- which shared functions are reused from multiple sites
- how function calls bind back to code

## Required Per-Feature Sections

Every feature function-map doc must contain:

- feature id
- owner crate
- owner module
- owner entry symbols
- request mainline
- response mainline
- error mainline
- shared multi-reference functions
- function call table
- sync status against code

## Code Binding Rule

Every call-table row should bind to code with:

- symbol path
- file path
- responsibility
- input semantic
- output semantic
- caller
- callee

If implementation is not landed yet, mark the row as binding pending. Do not invent symbols.

## Shared Multi-Reference Function Rule

If one function is called from multiple places, document it once here with:

- canonical symbol path
- owner
- shared purpose
- allowed callers
- related tests
- why it is shared instead of duplicated

## Update Rule

When code changes:

- update the call table
- update request/response/error mainlines
- update shared-function descriptions
- update sync status

If these are not updated, the feature is not closed.
