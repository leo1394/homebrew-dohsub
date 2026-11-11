# Release checklist

The version is recorded in `VERSION.txt`. Publishing is a separate operation from updating local version metadata.

## Shared publisher readiness

The repository uses `master`; the HEAD Formula builds that branch. Run the shared publisher from the parent Homebrew directory:

```bash
bash publish.sh --target homebrew-dohsub --dry-run
```

The current shared publisher rejects this Rust project with `unsupported release structure; expected bin/dohsub or go.mod + cmd/dohsub/main.go`. Its supported strategies cover interpreted scripts and Go, so `--prepare` / `--apply` are not ready for dohsub. Do not disguise the Rust executable as a script or a Go project.

Required integration: add a Rust release strategy that synchronizes `VERSION.txt`, Cargo.toml/Cargo.lock and the release date; runs Cargo, behavior and completion checks; and builds the four platform binaries listed below with matching `SHA256SUMS` and install.sh assets. Formula finalization must use a real source archive checksum. Validate the adapter with an isolated fixture before running `--prepare`; release publishing remains a separate action.

## Release checklist

1. Confirm the target repository is `leo1394/homebrew-dohsub` and review files for secrets. Keep local private state out of Git.
2. Set a stable SemVer in `VERSION.txt`, match the `Cargo.toml` version and update `src/main.rs` release date, refresh Cargo.lock, generate `man/dohsub.1` from that same build, and update documentation consistently. The build rejects mismatched version metadata. No formatting tools are required.
3. Build and test macOS arm64/x86_64 and Linux arm64/x86_64 targets. Document actual minimum macOS deployment targets and Linux glibc requirements. Do not claim old-platform support based only on install.sh.
4. Build `--release --locked` binaries named `dohsub-aarch64-apple-darwin`, `dohsub-x86_64-apple-darwin`, `dohsub-aarch64-unknown-linux-gnu`, `dohsub-x86_64-unknown-linux-gnu`. They each embed completion and manual generation.
5. Attach those binaries, `install.sh`, and `SHA256SUMS` to immutable tag `vX.Y.Z`. Checksum lines are `64-hex-sha256  exact-asset-name`, including install.sh. Resolve release metadata once per installer run; no moving-branch assets.
6. Add the stable source archive URL and its real SHA256 to `Formula/dohsub.rb`, retain HEAD support, and verify Formula version matches the tag. Do not use a fake checksum or claim HEAD is a released stable version.
7. Test source, local-binary, pinned remote, piped installer, upgrade and rollback in isolated prefixes. Confirm both SHA256 failures and candidate version mismatches preserve the old executable. Installer changes resources before binary replacement; resource failures require clear recovery instructions.
8. Run `brew install` and `brew test leo1394/dohsub/dohsub` in an authorized isolated target. Ruby syntax alone is insufficient. Confirm installed Bash/Zsh/Fish completion behavior and `man dohsub` rendering.
9. Exercise Homebrew ownership, upstream release ahead of Formula, target changes after confirmation, and standalone verified update. The updater never writes Cellar files. It requires the standalone `share/dohsub/install.json` marker and reuses its local installer.
10. Review all README claims against actual verification. Upgrade checks are stable-only; prereleases are not selected. A new binary does not hot-replace a running daemon: run `dohsub restart` after upgrade to apply it to the service.
