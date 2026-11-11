# Local validation — 2026-11-11

Platform: current Apple Silicon macOS, Rust/Cargo 1.94.0. Package version `0.1.1`, matching `VERSION.txt`. Source, fixtures, generated help/manual and documentation use provider-independent examples; DoH endpoints and node credentials are supplied by the user.

Passed:

- `cargo test --offline --locked`: **8 tests**, covering command contract, imports, provider-independent JSON metadata, DNS wire validation, port reservation, retry bounds and SemVer precedence.
- `cargo clippy --offline --locked -- -D warnings`: clean; no formatter was run.
- Version metadata: `VERSION.txt`, Cargo package/lock metadata and generated manual match `0.1.1`. The build-time guard accepts matching values and rejects mismatches; `version` and `--version` use the same two-line output. Unit/behavior checks and Clippy were rerun for this version.
- `cargo build --release --offline --locked`: successful; stripped release binary approximately **1.6 MB** on this target.
- `DOHSUB_TEST_BINARY=target/release/dohsub python3 tests/behavior.py`: **62 assertions**, using private temporary state, synthetic credentials, mocked curl/DoH/GitHub/Homebrew and loopback listeners.
- Behavior coverage: missing/unreachable DoH, explicit/default port collision, partial and duplicate imports, Unicode malformed input, private permissions, staging and y/no/EOF confirmations, negative and transient retry schedules, persisted retry deadlines, refresh bypass, passive subscription reads, remove/restore, local clipboard mocks, Host/Origin/token controls, empty HTTP 503, restart, persistence failure rollback, standalone installation, checksum and version mismatch, piped/pinned mocked downloads, upgrade cancellation, Formula lag/ownership, verified upgrade and post-install executable rollback.
- `DOHSUB_TEST_BINARY=target/release/dohsub python3 tests/completion.py`: **23 assertions**, real Bash function candidates and interactive Zsh Tab completion, including nested help, options/enums, quoted paths/spaces, repeated path options, non-path values and `--`. No application state was initialized. Zsh uses only the generated temporary function and OS-owned functions.
- Isolated `install.sh --source --prefix TEMP`: build, installed executable/version, generated resources verified. Normal local-binary installation was also tested.
- Installed `man -w dohsub` resolves and mandoc renders the manual. Bash/Zsh completion syntax, installer Bash syntax and Formula Ruby syntax pass.
- README local links and SVG XML validated. Original SVG logo/banner previewed in a separate temporary browser profile, with logo on light and dark backgrounds.

The final synthetic-node release-daemon RSS sample was **3200 KiB**; earlier samples varied. This is a single local observation, not a resource ceiling or a benchmark across workloads. DNS queries and filesystem syncs have transient overhead. Tests stopped their own daemons and removed temporary state; Existing local services and QX settings were not migrated or changed.

Unverified:

- Fish candidate execution: shell unavailable; generation was checked and the installer deploys its file.
- Linux, Intel Mac and older macOS/runtime compatibility.
- Full `brew install` / `brew test`; only Formula syntax and mocked ownership/update behavior were checked.
- Real GitHub release downloads/tap installation: no repository/release was published by this task.
- Live provider DoH, real proxy handshake and QX subscription import. Acceptance/publication does not claim proxy connectivity. No real node secrets were used in tests.
