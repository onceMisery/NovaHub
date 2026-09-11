# MVP 实施证据

## Slice 0

本记录初始化于 Task 1 开始前。

## Slice 1: Task 1 Workspace

- RED: `cargo test --manifest-path architecture-tests/Cargo.toml`，因根 `Cargo.toml` 不存在失败。
- GREEN: `cargo metadata --no-deps --format-version 1` 成功。
- GREEN: `cargo fmt --all -- --check` 成功。
- GREEN: `cargo clippy --workspace --all-targets -- -D warnings` 成功。
- GREEN: `cargo test --workspace` 成功，架构边界测试 1 passed。
- Residual risk: 当前仍只有 workspace 骨架，尚未包含真实 Slint、SQLite、Wasmtime 或平台 API。

## Slice 2: Task 2 Contracts

- RED: `cargo test -p novahub-core-domain` 因 Query/Action/QueryId 缺失失败；`cargo test -p architecture-tests` 因 WIT 文件缺失失败。
- GREEN: `cargo test -p novahub-core-domain -p novahub-ui-protocol -p novahub-ipc -p novahub-plugin-manager -p architecture-tests -- --nocapture` 通过。
- GREEN: `cargo fmt --all -- --check` 通过。
- GREEN: `cargo clippy --workspace --all-targets -- -D warnings` 通过。
- GREEN: `cargo test --workspace` 通过。
- Contract files: `crates/core-domain/src/lib.rs`、`crates/ui-protocol/src/lib.rs`、`crates/ipc/src/lib.rs`、`crates/plugin-manager/src/lib.rs`、`wit/novahub-plugin/plugin.wit`。
- Residual risk: WIT 尚未接入 wit-bindgen/Wasmtime；View 目前只实现 Empty/List，尚未覆盖 Grid/Detail/Form/ActionPanel。

## Slice 3: Task 3/4 Baseline And Shell Bridge

- GREEN: `tests/prototype/run.cjs` passed with system Chrome; report was `{"boards":35,"errors":[]}`.
- RED: `cargo test -p novahub-ui-slint` initially failed because `ShellState` and `StableList` were absent.
- GREEN: `cargo test -p novahub-ui-slint -p architecture-tests --offline -- --nocapture` passed after the bridge and `crates/ui-slint/ui/shell.slint` were added.
- GREEN: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --offline -- -D warnings`, and `cargo test --workspace --offline` passed.
- Decision: continue. Real Slint code generation is deferred until the renderer dependency set is cached and lockable; no WebView/Tauri fallback is introduced.

## Slice 4: Task 5 Search Generation Gate

- RED: `cargo test -p novahub-search --offline -- --nocapture` initially failed because `SearchCoordinator` and `SearchResult` were absent.
- GREEN: `cargo test -p novahub-search --offline -- --nocapture` passed with stale-generation rejection and bounded result tests.
- GREEN: workspace format, Clippy, and tests passed offline after the search slice.
- Residual risk: provider concurrency, cancellation tokens, deadline propagation, ranking weights, and versioned index snapshots remain pending.

## Slice 5: Task 6 Platform Capability Boundary

- RED: `cargo test -p novahub-platform-api --offline -- --nocapture` initially failed because capability types were absent.
- GREEN: `cargo test --workspace --offline` passed with explicit available/unavailable capability tests.
- GREEN: `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets --offline -- -D warnings` passed.
- Residual risk: Windows/macOS adapters, cancellation around blocking OS calls, DPI/multi-display behavior, and real permission prompts remain pending.

## Slice 6: Task 7 Migration Boundary

- RED: `cargo test -p novahub-migration --offline -- --nocapture` initially failed because `MigrationPlan` and `MigrationStep` were absent.
- GREEN: migration order, contiguous version validation, and reverse rollback tests passed.
- GREEN: `cargo clippy -p novahub-migration --all-targets --offline -- -D warnings` passed.
- Residual risk: real SQLite transactions, WAL/busy-timeout, encryption, quota/TTL and rollback against a database are not implemented yet.

## Slice 7: Task 8 Plugin Session Boundary

- RED: `cargo test -p novahub-plugin-runtime --offline -- --nocapture` initially failed because `PluginSession`, `SessionId`, and `SessionError` were absent.
- GREEN: session revision monotonicity, stale event rejection, and close rejection tests passed.
- GREEN: workspace Clippy passed after the runtime slice.
- Residual risk: Wasmtime Store fuel/epoch limits, WIT generated bindings, capability broker IPC, host process supervision, and crash recovery remain pending.

## Slice 8: Task 9 Plugin Install Boundary

- RED: `cargo test -p novahub-plugin-manager --offline -- --nocapture` initially failed because permission and install transaction APIs were absent.
- GREEN: explicit permission set, SemVer compatibility, staging-to-active commit, and transaction state tests passed.
- GREEN: `cargo clippy -p novahub-plugin-manager --all-targets --offline -- -D warnings` passed.
- Residual risk: archive validation, signature/hash checks, Zip Slip/zip bomb limits, atomic filesystem pointer, and uninstall cleanup remain pending.

## Slice 9: Task 10 Main Process Local Path

- RED: `cargo test -p novahub-app --offline -- --nocapture` initially failed because `NovaHubApp` was absent.
- GREEN: built-in command search test passed.
- GREEN: `cargo run -p novahub-app --offline` printed `NovaHub host bootstrap (1 builtin command)`.
- Boundary: app links core-domain/providers/search/ui-slint only; architecture test continues to reject Plugin Host/runtime dependencies in the main app manifest.

## Final Verification For Current Handoff

- `cargo fmt --all -- --check` passed.
- `cargo clippy --workspace --all-targets --offline -- -D warnings` passed.
- `cargo test --workspace --offline` passed.
- `cargo run -p novahub-app --offline` passed.
- `git diff --check` passed; only LF/CRLF normalization warnings were reported by Git.
- No references to the deleted historical review documents remain in the active architecture index.
- No UTF-8 BOM found in source, documentation, or configuration files.

## Slice 10: Task 7 Storage And Built-ins

- GREEN: SQLite settings round-trip, idempotent schema migration, atomic command snapshot and clipboard metadata TTL/sensitive rejection tests passed.
- GREEN: eight unique built-in provider families, calculator recursive-descent evaluation, safe system-command confirmation and SHA-256 clipboard hashing tests passed.
- Residual risk: system credential-backed encryption, real Windows Search/Spotlight adapters, image payload limits and provider E2E remain pending.

## Slice 11: Task 8/9 Host, SDK And Examples

- GREEN: IPC bounded envelope and unknown-field compatibility tests passed.
- GREEN: independent `novahub-plugin-host` session router tests passed; stdin/stdout framed process loop is implemented.
- GREEN: plugin TOML SemVer/kind parser, permission set, archive path guard, pet descriptor limits and transactional install tests passed.
- GREEN: Rust SDK capability guard and official JSON/translation example view tests passed.
- Residual risk: Wasmtime Component execution, fuel/epoch limits, signature/archive extraction, real install/update/uninstall filesystem lifecycle and WIT generated bindings remain pending.

## Slice 12: Task 10/Desktop-Pet Core Flow

- GREEN: migration parser rejects scripts/credentials and CLI dry-run performs zero writes on fixture.
- GREEN: official View protocol now covers List/Grid/Detail/Form/ActionPanel and WIT source declares the same variants.
- GREEN: host-owned PetController visible/shelf/working/success transitions and illegal-transition rejection passed.
- Residual risk: actual Slint rendering, platform overlays, tray/global hotkey, pet assets and accessibility/performance evidence remain pending.

## Slice 13: Prototype And Host Process Probes

- GREEN: `tests/prototype/run.cjs` with system Chrome returned `{"boards":35,"errors":[]}`.
- GREEN: built `novahub-plugin-host` and sent a real length-prefixed Protobuf frame over stdin; process replied `{"type":"ack","revision":1}` over stdout.
- Residual risk: no Wasmtime component loaded in the process probe yet; resource RSS/cold-start and crash-restart measurements remain pending.

## Slice 14: Architecture Drift Guards

- GREEN: architecture tests now assert WIT List/Grid/Detail/Form/ActionPanel variants, no Tauri/WebView main-shell dependency, and presence of two official example plus three desktop-pet manifests.

## Slice 15: Bounded Host IPC

- GREEN: Plugin Host rejects a declared frame larger than `MAX_ENVELOPE_BYTES` before allocating the frame buffer.
- GREEN: Host responses preserve the incoming Protobuf `request_id`; session errors now use stable codes `stale_revision` and `session_closed`.
- GREEN: `cargo fmt --all`, `cargo clippy --workspace --all-targets --offline -- -D warnings`, `cargo test --workspace --offline`, and `cargo run -p novahub-app --offline` passed.
- GREEN: rebuilt `novahub-plugin-host` and completed a real stdin/stdout probe with `request_id=req-42`; response frame size was 61 bytes.
- GREEN: final prototype regression returned `{"boards":35,"errors":[]}` using the bundled Playwright runtime and system Chrome.
- GREEN: 102 tracked source/config/document files were checked for UTF-8 BOM; none found. Deleted review-document names have no active references.
- Residual risk: Wasmtime Component execution, generated WIT bindings, archive extraction/signature verification, real Slint rendering, platform adapters, and RSS/cold-start measurements remain pending.

## Slice 16: Verified Plugin Install CLI

- GREEN: `novahub-plugin-manager` signed archive install test accepts a valid Ed25519 signature and rejects tampered archive bytes before ZIP parsing/extraction.
- GREEN: `novahub-cli` now requires `--signature <file>` and `--public-key <file>` for normal plugin installation; mismatched or incorrectly sized raw key files fail before installation.
- GREEN: `--allow-unsigned` is an explicit developer-mode escape hatch and is passed as `None` signature verification only when requested.
- GREEN: `cargo test -p novahub-plugin-manager --offline` (8 passed) and `cargo clippy -p novahub-plugin-manager --all-targets --offline -- -D warnings` passed.
- Residual risk: the current archive pointer replacement still needs platform-specific atomicity verification; release signing, key rotation/revocation, real Wasmtime validation, and platform installer integration remain pending.

## Slice 17: Wasmtime Component Execution

- GREEN: `tests/fixtures/component-plugin` builds as a real `wasm32-wasip2` Component after declaring an isolated fixture workspace.
- GREEN: WIT-generated guest bindings and Host-side `wasmtime::component::bindgen!` agree on `View`; WIT exports use `open-view` and `close-plugin` to avoid WASI libc symbol collisions.
- GREEN: Plugin Host registers WASI P2 imports only inside the runtime Store, with per-Store 64 MiB memory, fuel, epoch deadline, and bounded input/file reads.
- GREEN: `NOVAHUB_COMPONENT_FIXTURE=<path> cargo test -p novahub-plugin-host --offline component_fixture_executes_when_configured -- --nocapture` passed.
- GREEN: real stdin/stdout Protobuf process probe returned `loaded`, List View, Detail View, and `ack` responses with preserved request IDs.
- Residual risk: Windows/macOS platform adapters, RSS/cold-start measurements, signed archive extraction, and crash-restart evidence remain pending.

## Slice 18: Component Validation And App Entry

- GREEN: `cargo test -p novahub-plugin-manager --offline -- --nocapture` passed with 9 tests, including rejection of a core Wasm module disguised as `plugin.wasm`.
- GREEN: `cargo test -p novahub-app --offline -- --nocapture` passed with 5 tests after routing the configured fixture through `NovaHubApp::run_component_fixture`.
- GREEN: `cargo clippy -p novahub-app -p novahub-plugin-manager --all-targets --offline -- -D warnings` passed after splitting the Shell entrypoint into readable handlers.
- GREEN: `cargo build -p novahub-plugin-host --offline` plus `NOVAHUB_HEADLESS=1 NOVAHUB_PLUGIN_HOST=<host> NOVAHUB_COMPONENT_FIXTURE=<fixture> cargo run -p novahub-app --offline` returned `NovaHub plugin fixture completed (revision 2)` and `NovaHub host bootstrap (8 builtin command)`.
- Residual risk: Component validation confirms binary validity, while WIT export compatibility is still enforced at Host load; platform capability coverage, release signing/installers, resource sampling, and crash-restart evidence remain pending.

## Slice 19: Capability Default-Deny Correction

- GREEN: Windows and macOS adapter tests now assert `FileOpen` is the only implemented capability and Clipboard remains `Unavailable`.
- GREEN: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --offline -- -D warnings`, `cargo test --workspace --offline -- --test-threads=1`, `cargo run -p xtask -- release-check`, and `git diff --check` passed after the correction.
- Residual risk: actual Windows/macOS OS calls, permission prompts, global hotkeys, window management, accessibility, and resource baselines still require reference-machine evidence.
- Governance tooling note: `aegis-workspace.py bundle/check` could not run because the pre-existing workspace is missing `task-intent-draft.json`, `docs/aegis/adr`, and required index/governance entries; this does not affect the Rust verification commands above.

## Slice 20: Pet Window, Position Policy And Clipboard Vault

- GREEN: `cargo test -p novahub-core-domain -p novahub-ui-slint -p novahub-app --offline -- --test-threads=1` passed with the Slint `PetWindow`, logical display placement, and app position tests.
- GREEN: `ClipboardVault` tests passed for AEAD round-trip, sensitive-source rejection, text/image size limits, seven-day TTL, and 500-item LRU eviction.
- GREEN: `cargo clippy --workspace --all-targets --offline -- -D warnings` passed after documenting public fallible/panicking APIs and replacing implicit integer casts.
- GREEN: architecture docs now record PetWindow ownership, logical position policy, and ClipboardVault budgets; deleted historical review documents remain absent.
- GREEN: PetWindow exposes a fixed-size shelf summary and the main Shell routes `pet show`, `pet hide`, and click-to-shelf through `PetController`.
- GREEN: a credential/decryption failure no longer evicts the clipboard item; the regression test restores the key and successfully retries the same item.
- GREEN: optional pet-window show/hide failures now propagate as recoverable UI errors instead of panicking the main Shell; targeted app/UI tests and workspace Clippy pass afterward.
- GREEN: final `cargo test --workspace --offline -- --test-threads=1` passed after the error-propagation change.
- Residual risk: `CredentialStore` is an abstraction with an explicit memory test implementation; native Windows Credential Manager/macOS Keychain, clipboard listeners, native transparent overlay positioning, action-shelf dynamic binding, and RSS/GPU reference evidence are still pending.

## Slice 21: Application Snapshot And Clipboard Shell Loop

- GREEN: `cargo test -p novahub-app -p novahub-providers -p novahub-storage -p novahub-platform-windows -p novahub-platform-macos --offline -- --test-threads=1` passed, including host clipboard commands, application command identity, adjacent duplicate handling, and metadata deduplication.
- GREEN: `cargo clippy -p novahub-app -p novahub-providers -p novahub-storage -p novahub-platform-windows -p novahub-platform-macos --all-targets --offline -- -D warnings` passed.
- GREEN: `cargo fmt --all -- --check` passed.
- GREEN: `cargo clippy --workspace --all-targets --offline -- -D warnings` passed.
- GREEN: `cargo test --workspace --offline -- --test-threads=1` passed.
- GREEN: command matching tests passed after including command subtitles and de-duplicating same-name application results before stable-ID generation.
- GREEN: `NOVAHUB_HEADLESS=1 cargo run -p novahub-app --offline` returned `NovaHub host bootstrap (8 builtin command)`.
- GREEN: `cargo run -p xtask --offline -- release-check` passed and confirmed both retired review documents remain absent.
- GREEN: `git diff --check` reported no whitespace errors; the UTF-8 BOM scan reported `PASS`.
- Residual risk: platform adapters still require Windows/macOS reference-machine execution; the current app snapshot is bounded and explicit-refresh but does not yet receive native change notifications, and clipboard capture remains on-demand until a host scheduler/listener is added.

## Slice 22: Lightweight Shell Completion And Pet Resources

- GREEN: calculator tests cover `sqrt`, trigonometric/logarithmic helpers, constants, finite-result checks, and offline length/mass/time/temperature conversion with dimension mismatch rejection.
- GREEN: the desktop app opens `novahub.sqlite3` under the platform user data directory; `NOVAHUB_HEADLESS=1` continues to use the in-memory database for deterministic checks.
- GREEN: supported desktop builds register `Alt+Space` through `global-hotkey`; registration/backend failures are reported without stopping the Shell. The Shell query dispatcher is split into small domain-specific handlers and passes strict Clippy checks.
- GREEN: PetWindow binds host-owned SVG frames, exposes drag callbacks, and plugin-manager validates every resource declared by a desktop-pet `pet.json`. `xtask build-examples` packages Nova, Pixel, and Waterman pet archives.
- GREEN: `cargo fmt --all -- --check` passed; `cargo clippy --workspace --all-targets --offline -- -D warnings` passed; `cargo test --workspace --offline -- --test-threads=1` passed; `cargo run -p xtask --offline -- release-check` passed; `$env:NOVAHUB_HEADLESS='1'; cargo run -p novahub-app --offline` printed `NovaHub host bootstrap (9 builtin command)`; `cargo run -p xtask --offline -- build-examples` built five archives; `git diff --check` reported no whitespace errors.
- GREEN: UTF-8 BOM scan returned no files, and both retired review-document paths were confirmed absent.
- Residual risk: native Windows/macOS behavior, accessibility narration, installer signing/uninstall E2E, and reference-machine memory/GPU/cold-start evidence are not covered by offline workspace checks.

## Slice 23: Update, rollback, and active plugin execution

- GREEN: `cargo test -p novahub-plugin-manager --offline -- --test-threads=1` passed 12 tests, including versioned installation, persistent previous-pointer rollback, enabled-state preservation, signature checks, and pending-delete validation.
- GREEN: `cargo test -p novahub-app --offline -- --test-threads=1` passed 17 tests, including disabled active-plugin resolution without starting the Host and bounded View summaries.
- GREEN: `cargo check -p novahub-cli --offline` passed after adding `plugin update`, `plugin rollback`, and pending-delete reporting.
- Boundary: the application depends on `novahub-plugin-manager` only for validated package pointers; it still has no `wasmtime`, `plugin-runtime`, Tauri, or WebView dependency. Component execution remains in the sibling Host.
- Residual risk: signed installer UI, publisher display, and cross-platform update transport are not implemented; local signed archive verification remains the MVP trust boundary.

## Slice 24: Bounded application search improvements

- GREEN: application search now supports compact names and initial aliases over the existing 128-entry lazy snapshot, sorts by in-memory launch frequency, and increments usage only after a successful host launch. The usage map is capped at 128 entries, so no per-keystroke platform enumeration or unbounded resident index was introduced.
- GREEN: `cargo fmt --all -- --check` passed.
- GREEN: `cargo clippy --workspace --all-targets --offline -- -D warnings` passed.
- GREEN: `cargo test --workspace --offline -- --test-threads=1` passed all workspace unit and doc-test targets.
- GREEN: `cargo run -p xtask --offline -- release-check` passed; both retired review documents were absent and the app dependency boundary passed.
- GREEN: `$env:NOVAHUB_HEADLESS='1'; cargo run -p novahub-app --offline` passed with `NovaHub host bootstrap (10 builtin command)`.
- GREEN: headless real Component probe passed with `Plugin view updated to revision 2: Fixture update: {"source":"headless"}`.
- GREEN: `cargo run -p xtask --offline -- build-examples` produced JSON, translate, Nova, Pixel, and Waterman `.novahub-plugin` archives.
- GREEN: `git diff --check` passed; the UTF-8 BOM scan reported `UTF-8 BOM scan: PASS`.
- Residual risk: Windows/macOS reference-machine hotkey conflict, LaunchServices/Windows Search quality, native overlay positioning/DPI, accessibility, signed installers, and RSS/private-working-set/GPU/cold-start measurements remain external acceptance evidence.

## Slice 25: Cross-platform shortcut labels, geometry tests, and manifest summaries

- GREEN: `cargo test -p novahub-ui-slint -p novahub-plugin-manager -p novahub-app --offline -- --test-threads=1` passed after adding geometry boundary tests, macOS shortcut-label coverage, and manifest metadata compatibility coverage.
- GREEN: `cargo fmt --all -- --check` passed.
- GREEN: `cargo clippy --workspace --all-targets --offline -- -D warnings` passed after keeping numeric conversion allowances local to the display-geometry helpers.
- GREEN: `cargo test --workspace --offline -- --test-threads=1` passed all workspace unit and doc-test targets.
- GREEN: `cargo run -p xtask --offline -- release-check` passed; both retired review documents remain absent and the app dependency boundary passed.
- GREEN: `$env:NOVAHUB_HEADLESS='1'; cargo run -p novahub-app --offline` returned the host bootstrap line.
- GREEN: `cargo run -p xtask --offline -- build-examples` produced JSON, translate, Nova, Pixel, and Waterman `.novahub-plugin` archives.
- GREEN: `git diff --check` passed; the retired-document assertion passed; the UTF-8 BOM scan reported no files.
- Residual risk: platform-specific shortcut conflict behavior, native monitor geometry/DPI, LaunchServices/Windows Search, Narrator/VoiceOver, signed installer E2E, and reference-machine resource metrics remain unverified outside this Windows/offline environment.

## Slice 26: Interaction boundaries, install preview, and bounded Host timeout

- GREEN: `cargo test -p novahub-platform-windows -p novahub-platform-macos -p novahub-app -p novahub-plugin-manager -p novahub-ui-slint --offline -- --test-threads=1` passed: 20 app/plugin-client tests, 7 main Shell tests, 4 Windows adapter tests, 3 macOS adapter tests, 13 plugin-manager tests, and 11 UI tests.
- GREEN: Shell decision tests cover Escape priority (cancel confirmation, clear query/refocus, hide) and focus-loss behavior with pending confirmation.
- GREEN: `inspect_archive` tests cover valid bounded metadata, incompatible host versions, and malformed ZIP bytes; CLI preview remains read-only before signature-verified extraction.
- GREEN: Windows recursive `where.exe` fallback and macOS application-directory fallback are asserted disabled by default; explicit opt-in paths remain available for diagnostics.
- GREEN: A real silent Windows child process test waits for the Plugin Host client deadline and confirms the deterministic IPC timeout is returned without an unbounded UI-thread wait.
- GREEN: `cargo clippy -p novahub-platform-windows -p novahub-platform-macos -p novahub-app -p novahub-plugin-manager -p novahub-ui-slint -p novahub-cli --all-targets --offline -- -D warnings` passed.
- Residual risk: this environment still cannot prove native Windows/macOS API quality, global shortcut conflicts, Spotlight/Windows Search result quality, transparent overlay/DPI behavior, Narrator/VoiceOver, signed installer E2E, or RSS/private-working-set/GPU/cold-start targets. Aegis bundle/check remains blocked by pre-existing governance metadata gaps.

## Slice 27: Full regression and current architecture evidence

- GREEN: `cargo fmt --all -- --check` passed.
- GREEN: `cargo clippy --workspace --all-targets --offline -- -D warnings` passed.
- GREEN: `cargo test --workspace --offline -- --test-threads=1` passed for all workspace unit and doc-test targets.
- GREEN: `cargo run -p xtask --offline -- release-check` passed, including the app dependency boundary and absence of both retired review documents.
- GREEN: `cargo run -p xtask --offline -- build-examples` produced five archives: JSON Toolkit, Translate, Nova, Pixel, and Waterman.
- GREEN: `$env:NOVAHUB_HEADLESS='1'; cargo run -p novahub-app --offline` returned `NovaHub host bootstrap (9 builtin command)`.
- GREEN: `cargo run -p novahub-cli --offline -- migrate --dry-run tests/fixtures/migration-safe.json` returned `accepted=2, rejected=1, writes=0`.
- GREEN: `git diff --check` reported no whitespace errors; the UTF-8 BOM scan returned `PASS`.
- Boundary: the main Shell remains Rust + Slint and does not link Wasmtime, Plugin Runtime, Tauri, or WebView. WIT is still the sole plugin ABI, and Component execution remains in the on-demand sibling Plugin Host.
- Residual risk: this Windows/offline environment cannot prove macOS native behavior, reference-machine RSS/private-working-set/GPU/cold-start targets, platform accessibility narration, signed installer E2E, or production crash-restart evidence. These are release evidence gaps, not reasons to downgrade the main Shell to Tauri.

## Slice 28: File actions, custom pet activation, and release matrix

- GREEN: host file-result actions cover `files.open`, `files.reveal`, and `files.copy_path`; `clipboard.copy` remains host-owned. Tests confirm actions stay behind `PlatformServices` and do not expose file-manager or clipboard handles to plugins.
- GREEN: custom desktop pets load only after active-pointer, manifest, `pet.json`, and SVG resource revalidation. Tests cover bounded frames (2 MiB per frame and 8 MiB total), persistent `pet use <id>`, and Nova fallback for invalid resources.
- GREEN: `.github/workflows/ci.yml` defines Windows x86_64, macOS x86_64, and macOS arm64 matrices for fmt, Clippy, serial workspace tests, and release checks; tag builds publish unsigned artifacts for the protected signing/Notarization stage.
- GREEN: `cargo fmt --all -- --check` passed.
- GREEN: `cargo clippy --workspace --all-targets --offline -- -D warnings` passed.
- GREEN: `cargo test --workspace --offline -- --test-threads=1` passed for all workspace unit and doc-test targets.
- GREEN: `cargo run -p xtask --offline -- release-check` passed, including the app dependency boundary and absence of both retired review documents.
- GREEN: `cargo run -p xtask --offline -- build-examples` produced five archives: JSON Toolkit, Translate, Nova, Pixel, and Waterman.
- GREEN: `$env:NOVAHUB_HEADLESS='1'; cargo run -p novahub-app --offline` returned `NovaHub host bootstrap (10 builtin command)`.
- GREEN: `cargo run -p novahub-cli --offline -- migrate --dry-run tests/fixtures/migration-safe.json` returned `accepted=2, rejected=1, writes=0`.
- GREEN: `git diff --check` reported no whitespace errors; the UTF-8 BOM scan and retired-document scan both passed.
- Boundary: the main Shell remains Rust + Slint and does not link Wasmtime, Plugin Runtime, Tauri, or WebView. WIT is still the sole plugin ABI, and Component execution remains in the on-demand sibling Plugin Host.
- Residual risk: this Windows/offline run cannot prove macOS native behavior, reference-machine RSS/private-working-set/GPU/cold-start targets, shortcut conflicts, Spotlight/Windows Search quality, transparent overlay/DPI, Narrator/VoiceOver, signed installer E2E, or production crash-restart evidence. These are release acceptance gaps, not reasons to introduce a Tauri/WebView fallback.

## Slice 29: Release artifact, migration CLI, and resource report hardening

- GREEN: `.github/workflows/ci.yml` now copies the built `novahub-cli` binary and renames it to the user-facing `novahub`/`novahub.exe` artifact; no tag-package step references a nonexistent `novahub` Cargo target.
- GREEN: `cargo run -p xtask --offline -- release-check` verifies the CI artifact source names and passed.
- GREEN: `migrate --source utools --db state.sqlite3 export.json` argument scanning is covered by two `novahub-cli` unit tests; option values are no longer treated as the input file.
- GREEN: `cargo clippy -p xtask -p novahub-cli --all-targets --offline -- -D warnings` passed.
- GREEN: `cargo test -p novahub-cli -p xtask --offline -- --test-threads=1` passed five tests (CLI 3 + xtask 2), including option-value scanning, process-tree filtering, and Windows sample parsing.
- GREEN: `cargo run -p xtask --offline -- resource-report` emitted a Windows JSON report with recursive process count, RSS bytes, and Private Bytes.
- Residual risk: resource-report provides measurement plumbing, not reference-machine P95 evidence; macOS Private Working Set, GPU/texture memory, installer signing/Notarization, accessibility, and production crash-restart remain external acceptance work.

## Slice 30: Plugin command indexing and ID boundary hardening

- GREEN: official JSON Toolkit and Translate manifests declare bounded commands; the host indexes only enabled active plugins and emits canonical IDs in the `plugin:<plugin_id>:<command_id>` form. `cargo test --workspace --offline -- --test-threads=1` passed the app search-index regression alongside all existing workspace targets.
- GREEN: selected plugin commands resolve the active pointer, pass the validated `plugin.wasm` path to the sibling Plugin Host, preserve the initial `open` View for empty input, and send a bounded `update` only when input is present. The app still has no `wasmtime`, `plugin-runtime`, Tauri, or WebView dependency.
- GREEN: manifest parsing and all install/pointer/removal paths share the plugin ID guard; slash, backslash, colon, `..`, control characters, empty values, and overlong IDs are rejected. The plugin-manager test suite passed 14 tests.
- GREEN: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --offline -- -D warnings`, `cargo test --workspace --offline -- --test-threads=1`, `cargo run -p xtask --offline -- release-check`, `cargo run -p xtask --offline -- build-examples`, `$env:NOVAHUB_HEADLESS='1'; cargo run -p novahub-app --offline`, `cargo run -p novahub-cli --offline -- migrate --dry-run --source utools tests/fixtures/migration-safe.json`, `cargo run -p xtask --offline -- resource-report`, `git diff --check`, and the UTF-8 BOM scan all passed.
- GREEN: `release-check` confirms both retired technical review documents remain absent and verifies that CI archives copy `novahub-cli` to the user-facing `novahub`/`novahub.exe` names.
- Residual risk: the resource report is collection plumbing rather than reference-machine P95 evidence; native macOS behavior, accessibility narration, signed installers/Notarization, GPU memory, cold start, and crash-restart remain external acceptance work. Aegis bundle/check remains blocked by pre-existing governance metadata gaps.

## Slice 31: Non-blocking Plugin Host calls and bounded View rendering

- GREEN: Plugin Host calls from Shell actions run on a worker thread and return through a 16 ms Slint timer; the UI callback only resolves the validated active pointer and never waits for Wasmtime or IPC.
- GREEN: IPC envelopes carry a per-Host random authentication token; debug-only diagnostics report request IDs and byte lengths without printing token contents. Host authentication failures remain deterministic.
- GREEN: Component load/compile has a separate 30-second client budget, while `open/update/close` remain on a 2-second response budget and Wasmtime compute remains on its 2-second fuel/epoch deadline. A real Debug `wasm32-wasip2` Component round-trip completed through the app client after the budget split.
- GREEN: bounded `PluginViewSurface` kind/title/body values now enter the Slint Shell as a fixed-height accessible surface; loading, success, error, and normal-command transitions clear or replace the previous View.
- Boundary: the sibling Host remains short-lived by design: one user action owns one `load/open/update/close` session, then the Host exits. This keeps idle NovaHub memory free of Wasmtime; Host reuse is a measured future trigger, not a hidden fallback.
- Boundary: the MVP concurrency path uses `std::thread`, bounded `mpsc`, and Slint Timer; Tokio is not linked into the Shell until a real network/structured-async requirement justifies its startup and memory cost.
- Evidence: `cargo fmt --all`, `cargo check -p novahub-ui-slint -p novahub-app --offline`, and the headless Debug Component probe with `NOVAHUB_PLUGIN_HOST_DEBUG=1` passed; the probe logged matching request IDs and accepted responses for load/open/update/close.
- Residual risk: Debug cold-start compilation is still materially slower than Release and requires reference-machine P95 measurements; native platform behavior, accessibility narration, signed installers, and full process-tree RSS/private-working-set/GPU evidence remain external acceptance work.

## Slice 32: CLI scaffolding, package checks, and SDK View builders

- GREEN: `cargo fmt --all` and `cargo clippy -p novahub-cli -p novahub-sdk --all-targets --offline -- -D warnings` passed.
- GREEN: `cargo test -p novahub-cli -p novahub-sdk --offline -- --test-threads=1` passed all CLI argument and SDK builder/Mock Host tests.
- GREEN: `novahub plugin new <dir> --id com.example.smoke` created a self-contained scaffold; the generated project passed offline `cargo check`.
- GREEN: real `official.json-toolkit.novahub-plugin` passed `novahub plugin check`, and real `official.translate.novahub-plugin` passed `novahub plugin test`; both paths validated in temporary storage and left no active pointer changes.
- GREEN: `docs/04-plugin-platform.md` now documents the low-memory semantics of `new/dev/check/test`; the architecture plan records the actual standard-library thread/mpsc/Slint Timer stack instead of a stale Tokio target stack.
- Boundary: these commands and SDK builders add no resident runtime, watcher process, WebView, Tauri fallback, or alternate plugin ABI.
- Residual risk: `plugin test` currently proves package/Component contract validation, not a full interactive UI or production Host crash-restart test; official View interaction rendering and reference-machine cold-start/RSS/accessibility evidence remain external acceptance work. A 300-second offline `cargo build -p novahub-app --release` attempt timed out before producing `target/release/novahub-app.exe`; no build processes remain.

## Slice 33: Official View interaction loop

- GREEN: `PluginViewSurface` carries bounded items, fields, and actions; the Slint Shell exposes fixed List/Grid slots, Form inputs/submission, and ActionPanel buttons without dynamic layout growth.
- GREEN: item activation, Action dispatch, field input, and form submission return through a fresh bounded Plugin Host call; the UI callback does not load Wasmtime or wait synchronously.
- GREEN: `cargo fmt --all -- --check` passed; workspace Clippy passed with `-D warnings`; workspace tests passed, including `view_surface_keeps_bounded_interactive_slots_for_host_rendering` and the app interaction tests.
- Boundary: WIT remains the sole plugin ABI and plugins cannot inject Slint, HTML, or a resident JavaScript runtime.
- Residual risk: Windows/macOS accessibility narration, transparent overlay/DPI behavior, and reference-machine RSS/GPU/cold-start measurements remain external acceptance evidence.

## Slice 34: Host-owned plugin lifecycle E2E

- GREEN: `plugin_install_call_disable_uninstall_lifecycle_stays_host_owned` passed: a signed archive was installed, its active pointer and indexed command were resolved, a real Component was executed by the sibling Host, disable removed the command, and uninstall removed the version directory.
- GREEN: `cargo test --workspace --offline -- --test-threads=1` passed all workspace unit and doc-test targets; the full run included 25 app tests and the lifecycle regression.
- GREEN: `cargo run -p xtask --offline -- release-check` passed; `cargo run -p xtask --offline -- build-examples` produced five archives; headless Shell bootstrap and migration dry-run passed; `git diff --check` and UTF-8 BOM scan passed.
- Boundary: the main Shell still has no Wasmtime, Plugin Runtime, Tauri, or WebView dependency; Component execution remains in a short-lived, on-demand sibling Plugin Host.
- Residual risk: Release App build, native Windows/macOS resource and accessibility measurements, signed installers, and production crash-restart remain external release acceptance items. These gaps do not justify a Tauri/WebView fallback.

## Slice 35: Final code-level verification pass

- GREEN: `cargo fmt --all -- --check` passed before the final documentation-only checkpoint; workspace Clippy and `cargo test --workspace --offline -- --test-threads=1` passed with all targets green.
- GREEN: `cargo run -p xtask --offline -- release-check` passed, including app dependency boundaries, CI artifact names, and absence of `docs/12-technical-review.md` and `docs/13-technical-analysis-report.md`.
- GREEN: `cargo run -p xtask --offline -- build-examples` produced five `.novahub-plugin` archives; headless app bootstrap returned `NovaHub host bootstrap (10 builtin command)`; migration dry-run returned `accepted=2, rejected=1, writes=0`.
- GREEN: `cargo run -p xtask --offline -- resource-report` emitted the Windows process-tree RSS/Private Bytes report, proving measurement plumbing without claiming a reference-machine target.
- GREEN: `git diff --check` reported no whitespace errors; the tracked-file UTF-8 BOM scan returned `PASS`; dependency search found no app-level Tauri/WebView/Wasmtime runtime link.
- Boundary: the main Shell remains Rust + Slint, idle memory remains free of Wasmtime, and only the short-lived sibling Plugin Host executes Components on demand.
- Residual risk: the earlier 300-second build window was insufficient, but the Release App now builds successfully; macOS behavior, native Windows/macOS APIs, accessibility narration, installer signing/Notarization, GPU memory, cold start, and crash-restart still need release-environment evidence.

## Slice 36: Release App artifact

- GREEN: `cargo build -p novahub-app --release --offline` completed successfully in about 5 minutes 33 seconds and produced `target/release/novahub-app.exe`.
- GREEN: `$env:NOVAHUB_HEADLESS='1'; .\\target\\release\\novahub-app.exe` exited successfully with `NovaHub host bootstrap (10 builtin command)`.
- Boundary: the optimized App artifact retains the same Rust + Slint Shell and does not add Tauri, WebView, or a resident Wasmtime runtime; Component execution remains delegated to the sibling Host.
- Residual risk: native Windows/macOS behavior, reference-machine RSS/Private Working Set/GPU/cold-start targets, accessibility narration, signed installer/Notarization, and production crash-restart remain external release evidence.

## Slice 37: Background host work and platform deadlines

- GREEN: application discovery now runs in a host-owned worker and publishes a bounded 128-entry snapshot; query keystrokes filter the snapshot in memory and no longer enumerate platform application locations synchronously.
- GREEN: clipboard platform reads run on a short-lived standard-library thread and return through a bounded channel consumed by a Slint timer. Pause/resume, adjacent deduplication, encryption, TTL/LRU bounds, and SQLite metadata remain host-owned.
- GREEN: file search runs on a worker with bounded result delivery. `files.open`, `files.reveal`, and `files.copy_path` remain short host actions behind the existing platform capability boundary.
- GREEN: Windows Search/`where.exe`, macOS `mdfind`, and application-index commands use a 5-second killable subprocess deadline. Plugin IPC response and Wasmtime compute budgets remain the existing 2-second limits.
- GREEN: `cargo fmt --all -- --check` passed.
- GREEN: `cargo clippy --workspace --all-targets --offline -- -D warnings` passed.
- GREEN: `cargo test --workspace --offline -- --test-threads=1` passed all workspace unit and doc-test targets, including the new application snapshot, file-search parser, clipboard worker, and platform deadline tests.
- GREEN: `cargo run -p xtask --offline -- release-check` passed and confirmed both retired technical review documents are absent.
- GREEN: `cargo run -p xtask --offline -- build-examples` produced JSON Toolkit, Translate, Nova, Pixel, and Waterman archives.
- GREEN: `cargo build -p novahub-app --release --offline` passed and `$env:NOVAHUB_HEADLESS='1'; .\\target\\release\\novahub-app.exe` returned `NovaHub host bootstrap (10 builtin command)`.
- GREEN: `cargo check -p novahub-platform-macos --target x86_64-apple-darwin --offline` and the corresponding `aarch64-apple-darwin` check both passed; the platform adapter itself is cross-target compilable.
- GREEN: the tracked-file UTF-8 BOM scan returned no files and the post-documentation `git diff --check` passed.
- Boundary: the main Shell remains Rust + Slint without Wasmtime, Plugin Runtime, Tauri, or WebView dependencies. Background workers are host-owned and bounded; Component execution remains in the on-demand sibling Plugin Host.
- Residual risk: full-workspace macOS cross-check remains blocked by the environment's missing `cc` tool for `libsqlite3-sys`; this is a toolchain limitation, while the macOS platform crate checks pass. Reference-machine RSS/private-working-set/GPU/cold-start, native behavior, accessibility, signed installer/Notarization, and production crash-restart evidence remain external release acceptance items; none justify a Tauri/WebView fallback.

## Slice 38: Bounded Shell interaction and response channels

- GREEN: fixed eight-slot search result rows expose stable IDs, selected state, click activation, keyboard navigation, Enter dispatch, and an accessible button role. Empty slots remain explicit and do not change panel geometry.
- GREEN: `Cmd/Ctrl+K` opens a bounded host-owned ActionPanel; Escape closes it before the existing Shell hide/back path. Action execution stays in the host callback boundary.
- GREEN: `clipboard copy <index>` decrypts and restores text history entries through host-owned platform services. Image entries return a deterministic unsupported response and are never converted into fake text payloads.
- GREEN: worker and Plugin Host response channels use `sync_channel(1)`, bounding queued responses while preserving Slint Timer delivery and non-blocking UI callbacks.
- GREEN: `cargo fmt --all -- --check` passed.
- GREEN: `cargo clippy --workspace --all-targets --offline -- -D warnings` passed.
- GREEN: `cargo test --workspace --offline -- --test-threads=1` passed all workspace unit and doc-test targets, including result-row, ActionPanel, clipboard-copy, and bounded-channel regressions.
- GREEN: `cargo run -p xtask --offline -- release-check` passed; both retired technical review documents remain absent.
- GREEN: `cargo run -p xtask --offline -- build-examples` produced JSON Toolkit, Translate, Nova, Pixel, and Waterman archives.
- GREEN: `$env:NOVAHUB_HEADLESS='1'; cargo run -p novahub-app --offline` returned `NovaHub host bootstrap (10 builtin command)`.
- GREEN: `cargo build -p novahub-app --release --offline` passed and `$env:NOVAHUB_HEADLESS='1'; .\\target\\release\\novahub-app.exe` returned the same bootstrap line with exit status 0.
- GREEN: `cargo run -p novahub-cli --offline -- migrate --dry-run --source utools tests/fixtures/migration-safe.json` returned `accepted=2, rejected=1, writes=0`.
- GREEN: `cargo run -p xtask --offline -- resource-report` emitted the Windows process-tree RSS/Private Bytes report; this confirms collection plumbing only.
- GREEN: `git diff --check` reported no whitespace errors and the tracked-file UTF-8 BOM scan returned `PASS`.
- Boundary: the main Shell remains Rust + Slint without Wasmtime, Plugin Runtime, Tauri, or WebView dependencies. WIT remains the only plugin ABI, and Component execution remains in the short-lived sibling Plugin Host.
- Residual risk: this Windows/offline run cannot prove macOS native behavior, platform accessibility narration, reference-machine RSS/private-working-set/GPU/cold-start P95, signed installer/Notarization E2E, or production crash-restart. Full workspace macOS cross-check remains blocked by missing `cc` in `libsqlite3-sys`; the platform crate cross-checks remain available.

## Slice 39: Form protection, bounded clipboard policy, and on-demand settings

- GREEN: plugin Form editing now has an explicit host-owned dirty flag. Focus loss and Escape preserve an edited form; the flag resets only when the view is replaced or the bounded Host response completes.
- GREEN: `ClipboardPolicy` bounds user-configurable history count and TTL. The default remains 500 items and seven days; invalid values are rejected, existing encrypted entries are trimmed, and settings survive reopening the host database.
- GREEN: Storage now applies clipboard settings, expiry tightening, and metadata count trimming in one SQLite transaction; the storage regression verifies settings and metadata stay aligned.
- GREEN: `SettingsWindow` is a separate Slint component created only after `Cmd/Ctrl+,`; it edits theme, global hotkey, clipboard TTL, and clipboard item limit through host setters. Save validates all values and restores Shell focus; invalid input stays in the settings surface.
- GREEN: `cargo fmt --all -- --check` passed; workspace Clippy passed with `-D warnings`; the final `cargo test --workspace --offline -- --test-threads=1` passed all targets, including the storage transaction regression, 29 app tests, 21 provider tests, and 11 UI tests.
- GREEN: `cargo run -p xtask --offline -- release-check` passed and confirmed both retired technical review documents are absent; `build-examples` produced five archives; Release App build and `NOVAHUB_HEADLESS=1` startup passed with `NovaHub host bootstrap (10 builtin command)`.
- GREEN: the final Release App rebuild and `NOVAHUB_HEADLESS=1` startup passed with `NovaHub host bootstrap (10 builtin command)`; `resource-report` emitted the Windows process-tree RSS/Private Bytes JSON, confirming collection plumbing only and not a reference-machine budget claim. `git diff --check` and tracked-file UTF-8 BOM scan passed.
- Boundary: the main Shell remains Rust + Slint with no Tauri, WebView, Wasmtime, or resident JavaScript runtime; the only plugin execution path remains the on-demand sibling Plugin Host using WIT.
- Residual risk: real Windows/macOS focus behavior, UI Automation/VoiceOver narration, shortcut conflict handling, native clipboard/LaunchServices behavior, reference-machine RSS/private-working-set/GPU/cold-start, signed installers/Notarization, and production crash-restart remain external release acceptance work. None requires a Tauri/WebView fallback.

## Slice 40: Clipboard window, shortcut rebinding, theme application, and readability

- GREEN: global hotkey registration is now retained in explicit host state. Settings save registers the replacement before unregistering the old binding and attempts rollback when replacement fails, allowing a conflict to be corrected from Settings.
- GREEN: the on-demand `ClipboardWindow` has eight fixed rows and bounded controls for type filtering, text preview, text copy, pause/resume, clear, pin/unpin, and returning focus to Shell. Encrypted image entries show a deterministic unsupported-preview message.
- GREEN: `ClipboardVault` stores `pinned` metadata, preserves pinned items through TTL cleanup, evicts unpinned entries first at the count limit, and migrates older SQLite schemas. UI copy and pin actions resolve stable clipboard item IDs rather than filtered ordinals.
- GREEN: saved theme state is applied immediately to Shell, search rows, ActionPanel, plugin view, and Settings. Application matching checks both display names and launch-path filenames for aliases.
- GREEN: clipboard callbacks are split by responsibility and run dependencies are grouped in `RunHandlerContext`; this keeps the host/UI boundary readable without adding a runtime or process.
- GREEN: `cargo check --workspace --offline`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --offline -- -D warnings`, `cargo test --workspace --offline -- --test-threads=1`, `cargo build -p novahub-app --release --offline`, Release headless startup, `cargo run -p xtask --offline -- release-check`, and `git diff --check` passed. The workspace test run included 31 app-library tests, 17 Shell-main tests, 23 provider tests, and 12 storage tests, including pinned-expiry and legacy-schema migration regressions.
- GREEN: `release-check` confirmed `docs/12-technical-review.md` and `docs/13-technical-analysis-report.md` remain absent; the main application still has no Tauri/WebView dependency and does not link Wasmtime.
- Boundary: Rust + Slint remains the main Shell; Wasmtime remains confined to the on-demand sibling Plugin Host and WIT remains the only plugin ABI. No fallback, watcher process, resident JavaScript runtime, Tokio dependency, or second protocol was introduced.
- Residual risk: Windows/macOS native focus and clipboard behavior, LaunchServices/Windows Search quality, DPI/transparent overlay behavior, Narrator/VoiceOver, reference-machine RSS/private-working-set/GPU/cold-start, signed installers/Notarization, and production crash-restart still require external release evidence. These gaps do not justify a Tauri/WebView fallback.

## Slice 41: Reusable fuzzy ranking and indexed application refresh

- GREEN: `crates/search` now uses a reusable low-level `nucleo-matcher` with one host-owned scratch buffer. Command title, subtitle, and ID are ranked with bounded output; the high-level thread-pool runtime is not introduced.
- GREEN: application aliases now promote exact display-name, launch-name, compact-name, pinyin, and complete-initial matches above sparse fuzzy command matches; the `Visual Studio Code`/`vsc` regression is covered.
- GREEN: macOS application discovery prefers the LaunchServices registration database and falls back to `mdfind` only when LaunchServices is unavailable. Directory traversal remains opt-in diagnostics only.
- GREEN: the Shell loads at most 128 applications in a background worker, filters the in-memory snapshot during typing, and schedules a 30-second low-frequency refresh without enumerating platform locations on the UI path.
- GREEN: `cargo fmt --all` and `cargo test -p novahub-search --offline -- --test-threads=1` passed; the app suite passed 32 library tests and 17 Shell-main tests after the ranking change.
- GREEN: after this slice, workspace `check`, workspace Clippy with `-D warnings`, serial workspace tests, Release App build, Release headless startup, `xtask release-check`, `git diff --check`, and the tracked-file UTF-8 BOM scan all passed. The macOS platform crate also cross-checked for both `x86_64-apple-darwin` and `aarch64-apple-darwin`.
- Boundary: Rust + Slint remains the main Shell; Wasmtime remains confined to the on-demand sibling Plugin Host, WIT remains the only plugin ABI, and no Tauri/WebView fallback or resident JavaScript runtime was added.
- Residual risk: macOS LaunchServices quality, reference-machine RSS/private-working-set/GPU/cold-start, native accessibility/focus behavior, signed installers/Notarization, and production crash-restart still require external release evidence.

## Slice 42: Kunkun contract adoption and interaction routing

- GREEN: installed command metadata now carries `PluginInteraction` in the host command registration; selected commands and Pet Action Shelf entries route `view` to the existing View session and `one-shot` to `load -> run -> Host shutdown` without creating a persistent View session.
- GREEN: one-shot results are rendered as bounded host text (512 characters), and the completed one-shot clears active Component and form state so no stale View event can reuse a reclaimed session.
- GREEN: `NovaHubApp` resolves interaction metadata from the enabled active pointer and rejects missing components, disabled plugins, unknown commands, and attempts to execute a View command through the one-shot path.
- GREEN: `novahub plugin new <dir> --interaction view|one-shot` emits an explicit manifest interaction and a matching Rust/WIT scaffold; the generated one-shot scaffold passed offline `cargo check`. Install preview now includes command interaction and permission before/after scope plus publisher reason.
- GREEN: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --offline -- -D warnings`, `cargo test --workspace --offline -- --test-threads=1`, and `cargo run -p xtask --offline -- release-check` passed. The test run includes the command identity/one-shot result bound, active manifest interaction lookup, real Host one-shot fixture, and all prior workspace regressions.
- GREEN: recursive Markdown local-link validation and docs UTF-8 without BOM scan passed; `docs/12-technical-review.md` and `docs/13-technical-analysis-report.md` remain absent.
- Boundary: the adopted ideas remain contract-level improvements from Kunkun: typed permissions and diff, explicit command interaction, bounded official View semantics, stable-ID renderer updates, CLI templates, and host diagnostics. Tauri/WebView, JavaScript production runtime, arbitrary iframe UI, watcher processes, and a second ABI remain excluded.
- Residual risk: the runtime permission broker still needs platform capability-call evidence; stable-ID diff is host-side and not yet a published plugin patch protocol; reference-machine RSS/private-working-set/GPU/cold-start, native accessibility, signing/Notarization, and production crash-restart remain external release acceptance items.
## Slice 43: Host-owned bounded diagnostics

- GREEN: `DiagnosticEventBuffer` keeps a fixed-capacity ring (default 256), evicts the oldest event at capacity, and exposes a bounded local preview plus a short native summary.
- GREEN: diagnostic fields are sanitized and type-shaped: error classes retain only a classification prefix, pointer state is an allow-list, and no API accepts plugin input, form values, file contents, or full paths/URLs.
- GREEN: `run_active_plugin` and `run_active_plugin_one_shot` record successful and activation/disabled/Host failure outcomes. Successful Component sessions carry the sibling Host PID; one-shot and pre-spawn failures remain explicitly represented without inventing a PID.
- GREEN: `diagnostics.view` is registered as a host-owned built-in command and displays a bounded summary in the native Shell without starting Plugin Host.
- GREEN: `cargo fmt --all`, `cargo clippy -p novahub-app -p novahub-providers --all-targets --offline -- -D warnings`, and `cargo test -p novahub-app -p novahub-providers --offline -- --test-threads=1` passed. The focused app suite includes 37 library tests and 18 Shell-main tests; provider tests include the diagnostics command registration regression.
- Boundary: the main Shell remains Rust + Slint without Wasmtime, Tauri, WebView, or resident JavaScript. The event ring is an observation surface only; it does not own permissions, plugin execution, or a background process.
- Residual risk: the permission broker is not yet wired to real capability imports/calls; stable-ID operations remain an internal renderer optimization; cross-platform resource/accessibility, signed installers/Notarization, and production crash-restart evidence remain external acceptance work.

## Slice 44: Capability broker contract

- GREEN: `CapabilityBroker` rechecks `EffectiveGrant` on every typed request and returns deterministic `NotGranted`, `ScopeExceeded`, `HandleMismatch`, or `InvalidOrigin` errors.
- GREEN: Clipboard read/write are separate operations; HTTPS runtime URLs are normalized to their authority and rechecked on every redirect; file access accepts only an opaque plugin/session-bound handle; Storage checks the requested total against the grant quota.
- GREEN: `cargo fmt --all` and `cargo clippy -p novahub-plugin-manager --all-targets --offline -- -D warnings` passed, together with the focused broker regression test.
- Boundary: no WIT import, host process, native path, WebView, Tauri dependency, or resident runtime was added in this contract slice.
- Residual risk: the WIT world still has no capability import, user-grant persistence is not yet carried into Plugin Host, and native Clipboard/HTTP/File/Storage adapters remain pending; these are explicit P0 follow-ups.

## Slice 46: Runtime capability IPC and Shell context propagation

- GREEN: `CapabilityAdapter` is now used by the WIT capability import; the sibling Plugin Host sends bounded capability requests to the App and validates protocol version, request ID and auth token on the nested IPC response.
- GREEN: `HostPluginCapabilityHandler` reconstructs a typed broker request for every call, rejects identity mismatch and missing grants with stable error codes, enforces storage quota, and reports HTTP/file/logging/notification adapters as explicit unsupported results rather than bypassing authorization.
- GREEN: installed-plugin Shell View, one-shot and follow-up form/item/action events retain `ActivePluginExecution`; fixture execution remains explicit and deny-by-default.
- GREEN: focused App tests passed 40 library tests and 19 Shell-main tests, including the new capability and execution-context regressions. The final `cargo fmt --all -- --check`, workspace `cargo check`, strict workspace Clippy, serial workspace tests, `xtask release-check`, `git diff --check`, and UTF-8 BOM scan all passed.
- Boundary: the main Shell remains Rust + Slint, Wasmtime remains only in the short-lived sibling Plugin Host, WIT remains the only plugin ABI, and no Tauri/WebView/JavaScript runtime or resident watcher was introduced.
- Residual risk: persistent user grant/revocation, real capability Component golden E2E, native file/HTTP adapters, and Windows/macOS reference-machine RSS/private-working-set/GPU/accessibility/signing/crash-restart evidence remain open. These do not justify a Tauri/WebView fallback.

## Slice 47: Persistent user grants and revocation

- RED/GREEN: schema regression first failed with version 1 and passed after adding idempotent schema version 2 plus `plugin_user_grants`.
- RED/GREEN: storage API tests first failed because grant read/write methods were absent, then passed with bounded plugin IDs, 64 KiB documents, non-negative timestamps, replacement and restart recovery.
- RED/GREEN: App lifecycle tests first failed because revoke/approve APIs were absent, then passed for initial approval, single-capability narrowing, full revocation, restart persistence and re-approval.
- GREEN: update expansion keeps the old user grant intersection and does not silently grant the new HTTP capability; malformed persisted JSON prevents execution instead of falling back to the declaration.
- GREEN: Shell regressions cover `plugin permissions`, `plugin revoke` and `plugin approve`; focused App/Storage/Plugin Manager tests and strict Clippy passed.
- Boundary: grant JSON is host-owned `SQLite` data, permission types remain owned by `plugin-manager`, and Plugin Host receives only the computed `EffectiveGrant` over existing authenticated IPC.
- Residual risk: the real capability Component golden E2E, typed host-owned KV operation, native file/HTTP adapters and external platform/release evidence remain open.

## Slice 48: Real capability Component E2E

- RED: with the existing fixture, the new installed-plugin E2E returned the input string instead of `storage:ok`, proving that no capability import was executed.
- GREEN: the fixture now calls the generated WIT `storage-write` import. With a 1,024-byte persisted grant, the real App/Host process test returns `storage:ok`; after revocation it returns `storage:error:capability not granted`.
- GREEN: `cargo run -p xtask --offline -- capability-e2e` built the `wasm32-wasip2` fixture, built `novahub-plugin-host`, ran the environment-bound App E2E and reported `capability-e2e: PASS`.
- GREEN: final workspace format, check, strict Clippy, serial tests, capability E2E, `release-check`, `git diff --check` and UTF-8 BOM scan all passed after the complete slice.
- GREEN: CI runs the command on Windows x86_64, macOS x86_64 and macOS arm64 runners; `release-check` fails if that step disappears.
- Boundary: the E2E uses the production WIT ABI, Wasmtime runtime, sibling Host executable, authenticated IPC and App `CapabilityBroker`; no mock transport, direct database access from Host, or resident process is introduced.
- Residual risk: uninstall/pending-delete grant cleanup, redirect-per-hop, opaque handle mismatch, session invalidation, typed KV payloads and external reference-machine release evidence remain open.

## Slice 49: Uninstall and pending-delete authorization lifecycle

- GREEN: `Storage::remove_plugin_user_grant` deletes a grant row only for terminal uninstall; explicit empty grants remain the representation for revocation and pending-delete.
- GREEN: `retry_pending_delete_ids` preserves the existing count API while returning successfully removed plugin IDs for authorization cleanup.
- GREEN: CLI uninstall freezes authorization before filesystem removal, removes the row only after `Removed`, and keeps the empty row for `PendingDelete`. Retry opens the host database first and cleans only IDs whose markers were removed.
- GREEN: CLI unit tests cover database path resolution, canonical empty grant serialization, terminal uninstall cleanup, retry cleanup, and same-ID reinstall isolation.
- GREEN: `cargo test -p novahub-cli -p novahub-storage -p novahub-plugin-manager --offline -- --test-threads=1` passed 8 CLI, 22 Plugin Manager, and 15 Storage tests.
- GREEN: a real CLI probe removed a temporary plugin directory and verified SQLite contains zero `plugin_user_grants` rows for that plugin ID.
- GREEN: `cargo fmt --all -- --check`, `cargo check --workspace --offline`, strict workspace Clippy, serial workspace tests, `xtask capability-e2e`, and `xtask release-check` all passed after the final edits.
- GREEN: `git diff --check` reported no whitespace errors; the repository text-file UTF-8 BOM scan passed; `docs/12-technical-review.md` and `docs/13-technical-analysis-report.md` remain absent.
- Boundary: the change adds no resident process, WebView, Tauri fallback, JavaScript runtime, Tokio service, direct Plugin Host database access, or second ABI.
- Residual risk: redirect-per-hop, opaque file-handle mismatch, session invalidation, typed KV payloads, native adapters, and external Windows/macOS resource/accessibility/signing/crash-restart evidence remain open.

## Slice 50: Closed-session capability E2E

- GREEN: `PluginHostClient` test coverage keeps the Host process alive after a close request, sends a follow-up update, and observes the stable `session_not_found` Host error.
- GREEN: `xtask capability-e2e` now runs both the closed-session rejection and storage grant/revocation Component tests; both passed through the production WIT, Wasmtime, sibling Host and authenticated IPC path.
- GREEN: focused App tests and strict App/xtask Clippy passed; the test-only transport hook is excluded from production builds and does not alter the normal close/shutdown path.
- GREEN: after the final Slice 50 edit, `cargo fmt --all -- --check`, strict workspace Clippy, serial workspace tests (including 44 App library tests), `xtask capability-e2e`, `xtask release-check`, `git diff --check`, and the UTF-8 BOM scan passed.
- Boundary: no resident Host, WebView, Tauri fallback, JavaScript runtime, second ABI, or direct Plugin Host storage access was introduced.
- Residual risk: redirect-per-hop, opaque file-handle mismatch, typed KV payloads, native adapters, and external Windows/macOS resource/accessibility/signing/crash-restart evidence remain open.

## Slice 51: Redirect-per-hop broker matrix

- GREEN: the Plugin Manager golden test authorizes two explicitly declared HTTPS hops independently, rejects an undeclared redirect target with `ScopeExceeded`, and rejects an insecure HTTP target with `InvalidOrigin`.
- GREEN: the matrix passed with strict Plugin Manager Clippy; no HTTP client or redirect-following process was introduced.
- Boundary: the broker remains the authorization owner; a future HTTP adapter must call it again for every received redirect target and may not treat the initial authorization as a chain grant.
- Residual risk: network adapter E2E, opaque file-handle mismatch, typed KV, native adapters, and external platform/release evidence remain open.

## Slice 52: Opaque file-token mismatch Component E2E

- GREEN: WIT `read-file` accepts one opaque token; Runtime binds it to the current `PluginIdentity` and never accepts a path or plugin-supplied owner identity.
- GREEN: the App checks the token against its host-owned issuance set before entering the native adapter boundary. An unknown token returns `capability_handle_mismatch`; no fallback path reaches the filesystem.
- GREEN: `xtask capability-e2e` rebuilt the `wasm32-wasip2` fixture and passed storage success/revocation, closed-session rejection, and forged opaque-token rejection through the production Host/IPC path.
- GREEN: workspace `cargo check` passed after the WIT, Runtime, App and fixture changes.
- Boundary: the file adapter remains explicit unsupported; Plugin Host receives no SQLite or filesystem ownership, and the main Shell still has no Wasmtime/Tauri/WebView dependency.
- Residual risk: host-issued valid-handle/native file read E2E, HTTP adapter E2E, typed KV, and external platform/release evidence remain open.

## Slice 53: Host-owned typed plugin KV

- GREEN: Storage schema version 3 creates `plugin_kv`; key/value bounds, namespace isolation, replacement, quota overflow rollback, and terminal namespace deletion are covered by Storage tests.
- GREEN: WIT `storage-put`/`storage-get` generated bindings carry typed byte values through Runtime, authenticated nested IPC, App capability authorization, and the App-owned SQLite adapter.
- GREEN: the real Component capability E2E writes and reads `greeting=hello`, returns `kv:ok:hello`, and confirms the row is persisted in the App database before revocation.
- GREEN: CLI uninstall/pending-delete tests cover transactional removal of grants and KV state; no Plugin Host database access was added.
- GREEN: targeted CLI/Plugin Manager/Storage tests, strict workspace Clippy, workspace check, and `xtask capability-e2e` passed after the typed KV changes.
- GREEN: final workspace format/check, strict Clippy, serial workspace tests (including 45 App, 23 Plugin Manager and 17 Storage tests), `xtask capability-e2e`, `xtask release-check`, diff check and UTF-8 BOM scan passed.
- Boundary: values are bounded bytes, storage quota is rechecked by the broker and enforced transactionally by Storage, and the short-lived Host remains the only Wasmtime owner.
- Residual risk: valid native file-handle/read adapter, HTTP adapter E2E, reference-machine resource/accessibility/signing/crash-restart evidence remain open.

## Slice 54: Kunkun adoption and MVP boundary closure

- GREEN: the authority docs now carry the adopted command, permission, host-owned UI, stable-ID, tooling, diagnostics and trust constraints; the reference analysis is not a competing architecture owner.
- GREEN: the MVP capability path is frozen as `WIT → sibling Host → authenticated IPC → App-owned broker → host-owned adapter`. HTTP and Files have explicit redirect, deadline, size, encoding and lifecycle bounds without adding a runtime or dependency.
- GREEN: the adoption checklist separates frozen design, implemented code and remaining P0/P1 acceptance work. It no longer reports complete View semantics or the full capability matrix as implemented.
- GREEN: `git diff --check`, stale-authority-wording scan, strict UTF-8 decoding/BOM scan and retired-document checks passed.
- GREEN: `cargo run -p xtask --offline -- release-check` passed the App dependency boundary, capability-E2E CI wiring and retired-document checks.
- GREEN: `cargo run -p xtask --offline -- capability-e2e` passed 2 real Component/Host tests covering storage authorization/revocation and closed-session rejection.
- Boundary: this slice changes documentation only. Rust + Slint remains the main Shell, WIT remains the only plugin ABI, and Wasmtime remains confined to the on-demand sibling Host.
- Residual risk: bounded HTTP and valid file-token success paths, full WIT/View semantics, real Slint virtualization, diagnostics export, and Windows/macOS resource/accessibility/signing evidence remain MVP work.

## Slice 55-57: Files/HTTP P0 closure and adoption sync

- GREEN: `pick-file → opaque token → read-file` 和两跳 HTTPS GET 已通过真实 Component、Wasmtime sibling Host、认证 IPC 与 App broker 往返；确定性 picker/HTTP 响应只在 App 测试边界注入。
- GREEN: 文件 token 在每次成功、Host 错误或重试失败的 session 结束后显式清理；错误 session 后不能读取旧 token。
- GREEN: HTTP 自动重定向关闭，每跳重新授权；不安全 scheme、URL credentials、零大小/零 timeout、超限 body 和 timeout 均有稳定拒绝或错误码。
- GREEN: `cargo clippy --workspace --all-targets --offline -- -D warnings` 通过。
- GREEN: `cargo test --workspace --offline -- --test-threads=1 --nocapture` 通过；App 50 个 library tests、Windows 5 个和 macOS 4 个平台 tests 均通过。
- GREEN: `cargo run -p xtask --offline -- capability-e2e` 通过 2 个真实 Component/Host 测试，并在同一 capability E2E 中覆盖 storage、typed KV、撤销、伪造/有效文件 token 与 HTTP redirect。
- GREEN: `cargo run -p xtask --offline -- release-check` 通过 App 依赖边界、E2E CI 接线和 `docs/12`/`docs/13` 退休检查；`cargo fmt --all -- --check`、`git diff --check`、UTF-8 无 BOM 和旧状态扫描通过。
- GREEN: `StableList` 的 100/1,000/10,000 行窗口化、selection/focus/scroll anchor 保留和诊断删减/JSON 导出必要测试通过；UI 13 个测试、App 51 个测试均通过。
- Boundary: 本轮仍没有把窗口算法发布成插件 Patch ABI；真实 Slint 控件和诊断页面只消费宿主模型，后续仍需参考机性能与可访问性证据。
- Boundary: App 依赖仍不含 Tauri/WebView/Wasmtime/JS runtime；`rfd` 与同步 `ureq + rustls` 仅在用户动作中按需使用，没有 Tokio 或常驻网络服务。
- Residual risk: macOS 实机编译/交互、真实公网 TLS、picker 取消、安装器签名、资源与无障碍证据仍需参考机。

## Slice 59: Official Form View semantics

- GREEN: WIT package version is 1.2.0 and defines typed `form-control`/`form-option` values plus bounded
  initial value, helper, error and option fields. Rust UI protocol validation rejects duplicate field IDs,
  malformed options, invalid select defaults and non-boolean toggle defaults.
- GREEN: Rust SDK builders cover text, password, select, checkbox and switch fields; the official translate
  Component uses select options with stable IDs and builds successfully for `wasm32-wasip2`.
- GREEN: Plugin Host and App convert the WIT payload into a bounded renderer-neutral surface. Unknown controls,
  duplicate options and invalid defaults are discarded or normalized before Slint receives them.
- GREEN: Slint exposes native `LineEdit`, `ComboBox`, `CheckBox`, `Switch` and determinate progress controls;
  the UI bridge uses a named `PluginFieldSlot` configuration instead of an overlong positional function call.
- GREEN: Focused necessary tests passed: App 54, Shell main 20, Plugin Host 5, SDK 3, UI protocol 6 and Slint
  13; strict targeted Clippy passed; both official Components passed `cargo check --target wasm32-wasip2`.
- GREEN: final `cargo fmt --all -- --check`, strict workspace Clippy, serial workspace tests,
  `xtask capability-e2e`, `xtask release-check`, `git diff --check`, UTF-8 without BOM and retired-document
  checks all passed. The first cold `capability-e2e` attempt exceeded the external 6-minute command limit;
  isolated fixture/Host/App stages all passed and a fresh full rerun reported `capability-e2e: PASS` in 123 seconds.
- Boundary: WIT remains the only plugin ABI, Form values remain host-owned, and no Tauri/WebView/JavaScript
  runtime, resident Host, Tokio service or second ABI was introduced.
- Residual risk: accessory, cursor pagination, reference-machine RSS/private-working-set/GPU/accessibility/
  cold-start measurements, signed installers and crash-restart evidence remain open.

## Slice 60: Typed Detail metadata

- GREEN: WIT package version is 1.3.0 and replaces string-only Detail metadata with the typed `text`, `link` and
  `tag` variant. `key` remains the bounded stable metadata identity and duplicate keys are rejected by the UI
  protocol validator.
- GREEN: Rust SDK preserves `metadata()` as a text convenience and adds `metadata_link`/`metadata_tag`; Plugin
  Host serializes the typed value as a tagged JSON object. The official JSON Toolkit Component emits a `tag` value.
- GREEN: App accepts only known metadata kinds, bounded values without control characters, unique keys and HTTPS
  links. Renderer-neutral Detail surfaces retain the kind and render type-aware text without giving plugins a URL
  opener or arbitrary native UI control.
- GREEN: targeted tests passed App 55, Shell main 20, Plugin Host 5, SDK 3, UI protocol 7 and Slint 13; strict
  workspace Clippy, serial workspace tests, both official wasm32-wasip2 checks, real capability E2E,
  `release-check`, `git diff --check` and UTF-8 BOM checks passed. A headless official JSON Component probe printed
  `format: #json` through the real sibling Host path.
- Boundary: WIT remains the only plugin ABI; package versioning is pre-stable and no compatibility ABI, WebView,
  Tauri, JavaScript runtime, resident Host or second metadata owner was introduced.
- Residual risk: accessory, cursor pagination/default action, reference-machine performance/accessibility/
  cold-start/resource evidence, signed installers and crash-restart checks remain open.

## Slice 61: Bounded item accessory

- GREEN: WIT package version is 1.4.0 and gives List/Grid items one optional typed accessory containing bounded
  status, badge, shortcut and host-owned icon resource ID. UI protocol validation rejects oversized values,
  control characters and accessory objects without visible semantics.
- GREEN: Rust SDK builders, Plugin Host serialization and App parsing preserve the typed fields without accepting
  arbitrary icon paths. Slint renders status, badge, shortcut and a lightweight icon marker in the existing six
  fixed slots through the named `PluginItemSlot` configuration.
- GREEN: the real capability Component fixture returns `Ready`, `FIXTURE`, `Enter` and `fixture.icon`; the App E2E
  asserts badge and icon ID after the production WIT → Wasmtime sibling Host → authenticated IPC path.
- GREEN: `cargo fmt --all`, strict workspace Clippy, serial workspace tests, `xtask capability-e2e` and
  `xtask release-check` passed. Relevant suites include App library 55, Shell main 20, UI protocol 8, SDK 3,
  Plugin Host 5 and Slint 13 tests.
- Boundary: accessory is an additive host-owned View semantic, not a second ABI or plugin-native UI surface. The
  main Shell still has no Wasmtime, Tauri, WebView or JavaScript production runtime, and Plugin Host remains
  on-demand.
- Residual risk: cursor pagination, ActionPanel default/secondary action semantics, reference-machine performance/
  accessibility/cold-start/resource evidence, signed installers and crash-restart checks remain open.

## Slice 62: Bounded cursor pagination

- GREEN: WIT package version 1.5.0 adds optional `next-cursor` fields to List/Grid views. UI protocol and SDK tests
  accept opaque cursors up to 256 control-free characters and reject blank, oversized or control-bearing values.
- GREEN: Plugin Host serializes the cursor and App drops malformed cursor values before they reach Slint. The host
  consumes the existing fixed-slot snapshot before enabling `Load more`; the next page replaces the current snapshot.
- GREEN: the real Component fixture returned `fixture-page-2`; a production `load_more` event carried the cursor
  through the sibling Host and returned the `fixture.page-2` item. `capability-e2e: PASS` completed in 141 seconds.
- GREEN: focused App/Shell/Host/SDK/protocol tests and strict workspace Clippy passed.
- Boundary: one page remains capped at 100 items and no page history, Patch ABI, resident worker or alternate runtime
  was added.
- Residual risk: ActionPanel default/secondary semantics and external reference-machine/release evidence remained open.

## Slice 63: Typed ActionPanel roles and native confirmation

- GREEN: WIT package version 1.6.0 adds the typed `default`/`secondary` action role. UI protocol validation requires
  exactly one default action, at most six actions and bounded unique IDs/titles; the SDK exposes readable helpers.
- GREEN: Plugin Host preserves role and destructive fields. App discards panels with missing, duplicate or unknown
  default roles; Slint exposes the validated default action to Enter and labels secondary/destructive actions explicitly.
- GREEN: destructive plugin actions are retained in a host-owned pending-confirmation state and are emitted only after
  native Confirm. Cancel/Escape clears the state, while focus loss keeps it pending without execution.
- GREEN: full serial workspace tests passed, including App library 57, Shell main 21, Plugin Host 5, SDK 3 and UI
  protocol 10. Strict workspace Clippy and both official Component checks passed; the final real Component E2E
  preserved `default`, `secondary` and `destructive` through WIT, Wasmtime sibling Host and authenticated IPC,
  reporting `capability-e2e: PASS` in 175 seconds.
- Boundary: the Shell remains Rust + Slint; WIT is still the only plugin ABI; Wasmtime remains in the on-demand sibling
  Host; no Tauri/WebView/JavaScript runtime, plugin-native confirmation UI or resident process was introduced.
- Residual risk: Windows/macOS reference-machine RSS/private-working-set/GPU/cold-start/accessibility/crash-recovery,
  signed installers and native platform smoke remain release work.

## Slice 64: Windows Release renderer, process tree, GPU sampling and Wasmtime cache

- GREEN: Windows Release 默认策略为 `winit-software`；software 后端首帧通过首次 Winit `RedrawRequested` 后的有界回调观察，GPU 后端保留 `AfterRendering` 路径。
- GREEN: Release App 与 Plugin Host 使用 Windows subsystem，最终资源报告进程树不包含 `conhost.exe`；应用发现从 `window.show()` 前移路径移除，改为事件循环启动后的惰性刷新。
- GREEN: 资源采样记录 RSS、Private Bytes、Private Working Set、GPU Dedicated/Shared Memory，并按 GPU engine 类型保留最大利用率；software 后端没有 GPU 分配时记录不可用，不跨 engine 相加。
- GREEN: Wasmtime 官方 `cache` feature 已启用，App/Host 显式传递用户数据目录 `cache/wasmtime`，上限 256 文件、128 MiB；最终缓存为 2 文件、约 121 KiB，未使用自定义预编译格式或 unsafe deserialize。
- GREEN: Release 报告 `target/reference-benchmark/windows-x86_64-release.json` 使用 20 samples、3 warmup、15 秒资源保持，`effective_policy=winit-software`。Shell 冷首帧 P50/P95 为 919.98/1383.12 ms，RSS 43.39 MiB；1 Host 为 96.27/111.35 ms，RSS 44.41 MiB；4 Host 聚合为 153.43/250.90 ms，RSS 108.97 MiB。Private Bytes/Private Working Set 三场景均有记录。
- GREEN: FemtoVG 对照约 109 MiB RSS、71 MiB GPU Shared、0.62% 3D engine 利用率；因此 software 默认与低内存目标一致。Wasmtime 缓存前后单 Host 专项 P95 从约 649~1021 ms 降至约 267 ms。
- GREEN: 2026-08-19 新鲜验证通过 `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --offline -- -D warnings`、`cargo test --workspace --offline -- --test-threads=1`、两个官方 `wasm32-wasip2` 插件检查、`cargo run -p xtask --offline -- capability-e2e` 和 `cargo run -p xtask --offline -- release-check`。
- GREEN: `git diff --check`、209 个仓库文件的严格 UTF-8 无 BOM 扫描与退休文档扫描通过。仓库内未找到 `aegis-workspace.py`，因此没有声称 helper bundle/check 已执行。
- Boundary: Shell 数值是冷启动首帧，不能等同于热快捷键 ≤100 ms；4 Host P95 是会话 ready 聚合，不能等同于单 Host 500 ms 门槛。没有引入 Tauri、WebView、JavaScript runtime、常驻 Host、Tokio 服务或第二 ABI。
- Residual risk: macOS renderer/资源/首帧、无障碍、签名安装器/Notarization、真实 picker/TLS 和 Host 崩溃回收仍缺发布实机证据；这些缺口不构成 Tauri fallback 的理由。
