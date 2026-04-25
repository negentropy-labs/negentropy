# Negentropy
### Code Negentropy for AI-driven refactoring

Deterministic feedback for AI-written codebases.

Negentropy analyzes the structural complexity of a repository and
produces actionable diagnostics that guide both humans and coding agents
to converge toward a healthier architecture.

Unlike linters or SAST tools, Negentropy does not enforce style rules.
It measures software entropy — coupling, ownership ambiguity, lifecycle
mismatch, and dependency topology — and turns them into executable
refactoring tasks.

## CLI MVP (V2 only)

Run analysis on a repository:

```bash
cargo run -- analyze . --format both
```

Scan with custom suffixes:

```bash
cargo run -- analyze . --extensions .ts,.tsx,.mts --format json
```

Generate and compare against a baseline report:

```bash
cargo run -- analyze . --format json --output baseline.json
cargo run -- analyze . --format both --baseline baseline.json
```

Default suffix list:

```text
.ts,.tsx,.js,.jsx,.mjs,.cjs,.mts
```

Exit codes:

- `0`: analysis completed without triggering `--fail-on`
- `2`: threshold hit by `--fail-on`
- `1`: argument or runtime error
