# Shared contracts

`source.py` is the sole editable type/schema/registry/command source, owned by C.
Run `python3 schemas/generate.py`; CI/local verification uses `--check`.
Generated JSON Schema 2020-12, TypeScript and serde Rust types are committed.
Required nullable fields must be present on the wire: use Schema validation before
serde decoding because Rust `Option<T>` alone does not enforce field presence.

- `validate.py` supplies source-side structural and semantic validation and an
  explicit legacy material adapter, consumed by the Python engine. Add the repo
  `schemas/` directory to Python's import path; do not copy the schema into E.
- Rust `focaus-contracts` exports wire types, validators, capability/credential
  policy, `HostServices`, and a fail-closed `StubHost`. It does not open devices,
  contact cloud services, execute arbitrary scripts, or provide a secret value.
- TypeScript `@focaus/contracts` exports wire types and `HostAdapter`. `mode` is
  explicit; fixture adapters cannot be selected during native acceptance.
- `fixtures/contracts/cases.json` has positive/negative schema cases and separately
  labelled semantic cases. `runtime-scenarios.json` describes required behavior
  for implementation; it is not a report that this behavior already works.
- `math-golden.json` contains independent hand-calculated examples; N implements
  algorithms, F consumes N's validated output. Golden data is synthetic.
- `search-golden.json` fixes Unicode 15.0.0 and profile `nfc_casefold_ws_v1`.
- Content hashes use RFC 8785 (including IEEE 754 number serialization). Resource
  hashes use raw bytes. Test vectors cover number formatting and non-ASCII.
- `x-host-only-commands` must never be exposed directly to a WebView. All native
  entry points authenticate the channel/actor, scope, revisions and tombstones.
- G1 database migration is schema **1**, independent of contract **2.0.0**.
  The connection must enable `PRAGMA foreign_keys=ON` before beginning migration;
  apply the script atomically. Reapplication is rejected; a migration registry
  controls version selection. Back up a live library before future upgrades.

Install Python verification dependencies in a local venv from `requirements.txt`.
Checks: generator `--check`; `python -m unittest discover -s tests/integration`;
`pnpm typecheck`; `pnpm test:contracts`; `cargo test --workspace`.

Four development chats may propose changes here but only C edits or regenerates.
G1 validates contracts and the initial migration, not native behavior or cloud
quality. Runtime adapter, UI, microphone and real-course validation remain G2–G5.
