#!/bin/bash
# Bash 3.2+. Standalone installation; no Homebrew or sudo required.
set -euo pipefail
repository=https://github.com/leo1394/homebrew-dohsub
prefix="${HOME}/.local"
version=""
binary=""
source_mode=0
usage() {
  cat <<'USAGE'
Usage: bash install.sh [--prefix DIR] [--version X.Y.Z] [--binary FILE | --source]
Default: download the latest stable release and verify its SHA256SUMS.
--source builds the local checkout using Rust 1.94+ and Cargo.lock.
--binary installs a trusted local binary without a download (no checksum provenance).
Default prefix: ~/.local. Supports macOS/Linux, arm64/x86_64.
USAGE
}
while [ "$#" -gt 0 ]; do
  case "$1" in
    --prefix|--version|--binary)
      [ "$#" -ge 2 ] || { echo "Missing value for $1" >&2; exit 2; }
      case "$1" in --prefix) prefix=$2;; --version) version=${2#v};; --binary) binary=$2;; esac
      shift 2;;
    --source) source_mode=1; shift;;
    --help|-h) usage; exit 0;;
    *) echo "Unknown option: $1; see --help" >&2; exit 2;;
  esac
done
[ -z "$binary" ] || [ "$source_mode" -eq 0 ] || { echo 'Choose --binary or --source' >&2; exit 2; }
case "$prefix" in /*) ;; *) echo '--prefix must be an absolute path' >&2; exit 2;; esac
case "$(uname -s):$(uname -m)" in
  Darwin:arm64) platform=aarch64-apple-darwin;;
  Darwin:x86_64) platform=x86_64-apple-darwin;;
  Linux:aarch64|Linux:arm64) platform=aarch64-unknown-linux-gnu;;
  Linux:x86_64) platform=x86_64-unknown-linux-gnu;;
  *) echo 'Unsupported platform; supported: macOS/Linux arm64/x86_64' >&2; exit 1;;
esac
mkdir -p "$prefix"
prefix=$(cd "$prefix" && pwd -P)
case "$prefix" in */Cellar/*) echo 'Refusing to overwrite a Homebrew installation; use brew upgrade' >&2; exit 1;; esac
stage=$(mktemp -d "$prefix/.dohsub-install.XXXXXXXX")
trap 'rm -rf "$stage"' EXIT
script_path="${BASH_SOURCE[0]:-}"
script_dir=""
if [ -n "$script_path" ] && [ -f "$script_path" ]; then script_dir=$(cd "$(dirname "$script_path")" && pwd -P); fi
fetch() { curl --fail --silent --show-error --location --proto '=https' --proto-redir '=https' --connect-timeout 5 --max-time 120 --output "$2" "$1"; }
if [ "$source_mode" -eq 1 ]; then
  [ -n "$script_dir" ] && [ -f "$script_dir/Cargo.toml" ] || { echo '--source requires a local project checkout' >&2; exit 1; }
  command -v cargo >/dev/null || { echo 'Install Rust 1.94+ before using --source' >&2; exit 1; }
  cargo build --release --locked --manifest-path "$script_dir/Cargo.toml"
  binary="$script_dir/target/release/dohsub"
fi
if [ -n "$binary" ]; then
  [ -f "$binary" ] || { echo 'Local binary is missing' >&2; exit 1; }
  cp "$binary" "$stage/dohsub"
else
  command -v curl >/dev/null || { echo 'curl is required' >&2; exit 1; }
  if [ -z "$version" ]; then
    fetch https://api.github.com/repos/leo1394/homebrew-dohsub/releases/latest "$stage/release.json"
    version=$(sed -n 's/^[[:space:]]*"tag_name":[[:space:]]*"v\{0,1\}\([0-9][0-9.]*\)".*/\1/p' "$stage/release.json")
  fi
  case "$version" in ''|*[!0-9.]*|.*|*..*|*.) echo 'No valid stable version found; use --source to build the local checkout' >&2; exit 1;; esac
  asset="dohsub-$platform"
  base="$repository/releases/download/v$version"
  fetch "$base/$asset" "$stage/dohsub"
  fetch "$base/SHA256SUMS" "$stage/SHA256SUMS"
  expected=$(awk -v name="$asset" '$2==name || $2=="*"name {print $1}' "$stage/SHA256SUMS")
  [ "${#expected}" -eq 64 ] || { echo 'Missing/invalid release checksum' >&2; exit 1; }
  if command -v shasum >/dev/null; then actual=$(shasum -a 256 "$stage/dohsub" | awk '{print $1}'); else actual=$(sha256sum "$stage/dohsub" | awk '{print $1}'); fi
  [ "$actual" = "$expected" ] || { echo 'Checksum mismatch; installation unchanged' >&2; exit 1; }
fi
chmod 755 "$stage/dohsub"
"$stage/dohsub" --version > "$stage/version" || { echo 'Candidate version check failed; installation unchanged' >&2; exit 1; }
[ "$(wc -l < "$stage/version" | tr -d ' ')" = 2 ] && [ "$(sed -n '2p' "$stage/version")" = "$repository" ] || { echo 'Invalid binary version output' >&2; exit 1; }
installed_version=$(sed -n '1s/^dohsub version \([^ ]*\) ([0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9])$/\1/p' "$stage/version")
[ -n "$installed_version" ] || { echo 'Invalid binary metadata' >&2; exit 1; }
[ -z "$version" ] || [ "$installed_version" = "$version" ] || { echo 'Candidate version mismatch; installation unchanged' >&2; exit 1; }
"$stage/dohsub" man > "$stage/dohsub.1" || { echo 'Manual generation failed; executable unchanged' >&2; exit 1; }
for shell in bash zsh fish; do "$stage/dohsub" completion "$shell" > "$stage/$shell" || { echo "Completion generation failed for $shell; executable unchanged" >&2; exit 1; }; [ -s "$stage/$shell" ]; done
[ -s "$stage/dohsub.1" ] || { echo 'Manual generation failed' >&2; exit 1; }
if [ -n "$script_dir" ]; then cp "$script_path" "$stage/install.sh"; else
  [ -n "$version" ] || { echo 'Piped local-binary installation requires a versioned installer file' >&2; exit 1; }
  fetch "$base/install.sh" "$stage/install.sh"
  expected=$(awk '$2=="install.sh" || $2=="*install.sh" {print $1}' "$stage/SHA256SUMS")
  if command -v shasum >/dev/null; then actual=$(shasum -a 256 "$stage/install.sh" | awk '{print $1}'); else actual=$(sha256sum "$stage/install.sh" | awk '{print $1}'); fi
  [ "$actual" = "$expected" ] && [ "${#expected}" = 64 ] || { echo 'Installer checksum mismatch' >&2; exit 1; }
fi
mkdir -p "$prefix/bin" "$prefix/share/man/man1" "$prefix/share/bash-completion/completions" "$prefix/share/zsh/site-functions" "$prefix/share/fish/vendor_completions.d" "$prefix/share/dohsub"
# Prepare every supporting resource before replacing the executable.
install -m 644 "$stage/dohsub.1" "$prefix/share/man/man1/dohsub.1"
install -m 644 "$stage/bash" "$prefix/share/bash-completion/completions/dohsub"
install -m 644 "$stage/zsh" "$prefix/share/zsh/site-functions/_dohsub"
install -m 644 "$stage/fish" "$prefix/share/fish/vendor_completions.d/dohsub.fish"
install -m 755 "$stage/install.sh" "$prefix/share/dohsub/install.sh"
printf '{"repository":"%s","version":"%s"}\n' "$repository" "$installed_version" > "$stage/install.json"
install -m 644 "$stage/install.json" "$prefix/share/dohsub/install.json"
previous="$prefix/bin/dohsub.previous"
if [ -e "$prefix/bin/dohsub" ]; then [ ! -L "$prefix/bin/dohsub" ] || { echo 'Refusing to replace a symlink executable' >&2; exit 1; }; cp -p "$prefix/bin/dohsub" "$previous"; fi
mv -f "$stage/dohsub" "$prefix/bin/dohsub"
if ! "$prefix/bin/dohsub" --version > "$stage/verified" || ! cmp -s "$stage/version" "$stage/verified"; then
  if [ -f "$previous" ]; then mv -f "$previous" "$prefix/bin/dohsub"; else rm -f "$prefix/bin/dohsub"; fi
  echo 'Post-install verification failed; previous executable restored. Supporting resources may need reinstalling.' >&2; exit 1
fi
printf 'Installed dohsub %s to %s/bin/dohsub\n' "$installed_version" "$prefix"
printf 'PATH: export PATH="%s/bin:$PATH"\n' "$prefix"
printf 'Manual: export MANPATH="%s/share/man:${MANPATH:-}"\n' "$prefix"
printf 'Bash: source "%s/share/bash-completion/completions/dohsub"\n' "$prefix"
printf 'Zsh: add %s/share/zsh/site-functions to fpath before compinit\n' "$prefix"
printf 'Fish: source "%s/share/fish/vendor_completions.d/dohsub.fish"\n' "$prefix"
