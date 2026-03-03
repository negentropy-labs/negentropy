# Fixture Repositories

These fixtures are small synthetic repositories used to validate V2 metrics.

- `01_mts_default`: includes only `.mts` source to verify default extension coverage.
- `02_extension_filter`: mixed `.ts/.mts/.js` files for extension filtering behavior.
- `03_cycle_tce`: explicit 3-node import cycle to trigger high TCE/TCR.
- `04_plme_deep_path`: deep `../../..` imports to push PLME.
- `05_edr_hardcoded`: hardcoded `new` dependency pattern to lower EDR.
- `06_ead_feature_envy`: functions that read external object attributes heavily to raise EAD.
- `07_sse_oa`: excessive mutable declarations and multi-writer member mutations for SSE/OA.
- `dimensions/`: seven V2 dimensions, each with `good` and `bad` fixture repos.
