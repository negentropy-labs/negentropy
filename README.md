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

`negentropy.toml` can set repository defaults. CLI flags still win:

```toml
[scan]
extensions = [".ts", ".tsx", ".mts"]
exclude = ["src/generated/**"]
include_tests = false
include_generated = false
include_migrations = false
include_benches = false

[privacy]
literal_payload = "redacted" # full | redacted | none
```

Discovery respects `.gitignore` by default.

Generate and compare against a baseline report:

```bash
cargo run -- analyze . --format json --output baseline.json
cargo run -- analyze . --format both --baseline baseline.json
```

Baseline comparison requires a matching analysis fingerprint: tool
version, target path, effective extensions, configuration digest, and
scanned file set must match. With `--baseline`, `--fail-on` gates on
regressions such as risk upgrades or new hotspots instead of the current
absolute risk level.

Default suffix list:

```text
.ts,.tsx,.js,.jsx,.mjs,.cjs,.mts
```

V2 metrics:

```text
IIE, EAD, TCR, TCE, EDR, PLME, SSE+OA,
VND, LDP, DIS, DMR, BFP
```

Exit codes:

- `0`: analysis completed without triggering `--fail-on`
- `2`: threshold hit by `--fail-on`
- `1`: argument, runtime, parse, or baseline compatibility error
