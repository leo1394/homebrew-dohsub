# Release checklist

The version is recorded in `VERSION.txt`. Publishing is a separate operation from updating local version metadata.

## Shared publisher readiness

The repository uses `master`. Stable Homebrew installs download verified precompiled binaries; only `--HEAD` builds that branch with Rust. Run the shared publisher from the parent Homebrew directory:

```bash
bash publish.sh --target homebrew-dohsub --dry-run
```

The shared publisher supports standard Cargo projects directly. The optional external profile at `../publish-profiles/homebrew-dohsub.json` selects the four-platform matrix, existing resource checks and `formula_mode: binary`. Keep release logic in the shared publisher; no project release scripts or CI workflow are required.

```bash
bash publish.sh --target homebrew-dohsub --version X.Y.Z --prepare
# Review, commit and push prepared metadata before publishing.
bash publish.sh --target homebrew-dohsub --apply
```

The publisher builds and verifies all assets before tagging, publishes the immutable release, then updates platform-specific Formula URLs and SHA256 values from the verified assets. Its source archive verification is a publisher integrity check; stable Homebrew users download only their platform binary. For recovery of an existing published release, use `--finalize-formula` without recreating or moving its tag.

## Release checklist

1. Confirm the target repository is `leo1394/homebrew-dohsub` and review files for secrets. Keep local private state out of Git.
2. Set a stable SemVer in `VERSION.txt`, match the `Cargo.toml` version and update `src/main.rs` release date, refresh Cargo.lock, generate `man/dohsub.1` from that same build, and update documentation consistently. The build rejects mismatched version metadata. No formatting tools are required.
3. Build and test macOS arm64/x86_64 and Linux arm64/x86_64 targets. Document actual minimum macOS deployment targets and Linux glibc requirements. Do not claim old-platform support based only on install.sh.
4. Build `--release --locked` binaries named `dohsub-aarch64-apple-darwin`, `dohsub-x86_64-apple-darwin`, `dohsub-aarch64-unknown-linux-gnu`, `dohsub-x86_64-unknown-linux-gnu`. They each embed completion and manual generation.
5. Attach those binaries, `install.sh`, and `SHA256SUMS` to immutable tag `vX.Y.Z`. Checksum lines are `64-hex-sha256  exact-asset-name`, including install.sh. Resolve release metadata once per installer run; no moving-branch assets.
6. Finalize the platform-specific release binary URLs and real SHA256 values in `Formula/dohsub.rb`. Retain HEAD-only Rust build dependencies and completion/manual generation. Stable installs must not require Rust/LLVM. A packaging-only change can increment `revision` without replacing the release assets; a new application version resets that revision.
7. Test source, local-binary, pinned remote, piped installer, upgrade and rollback in isolated prefixes. Confirm both SHA256 failures and candidate version mismatches preserve the old executable. Installer changes resources before binary replacement; resource failures require clear recovery instructions.
8. Run `brew install` and `brew test leo1394/dohsub/dohsub` in an authorized isolated target. Ruby syntax alone is insufficient. Confirm installed Bash/Zsh/Fish completion behavior and `man dohsub` rendering.
9. Exercise Homebrew ownership, upstream release ahead of Formula, target changes after confirmation, and standalone verified update. The updater never writes Cellar files. It requires the standalone `share/dohsub/install.json` marker and reuses its local installer.
10. Review all README claims against actual verification. Upgrade checks are stable-only; prereleases are not selected. A new binary does not hot-replace a running daemon: run `dohsub restart` after upgrade to apply it to the service.
