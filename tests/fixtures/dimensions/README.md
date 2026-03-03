# Dimension Fixtures (Good vs Bad)

Each V2 dimension has two fixture repos:

- `good`: expected to score better (lower risk) for the target dimension.
- `bad`: expected to score worse (higher risk) for the target dimension.

Dimensions:

- `module_abstraction`
- `logic_cohesion`
- `change_blast_radius`
- `architecture_decoupling`
- `testability_pluggability`
- `intent_redundancy`
- `state_encapsulation`

Use:

```bash
cargo run -- analyze tests/fixtures/dimensions/<dimension>/good --format json --fail-on none
cargo run -- analyze tests/fixtures/dimensions/<dimension>/bad --format json --fail-on none
```
