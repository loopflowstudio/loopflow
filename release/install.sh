#!/bin/sh
set -eu

REPO="loopflowstudio/loopflow"
DEFAULT_INSTALL_DIR="$HOME/.local/bin"

resolve_install_dir() {
  if [ -n "${LF_INSTALL_DIR:-}" ]; then
    echo "$LF_INSTALL_DIR"
    return
  fi

  if command -v lf >/dev/null 2>&1; then
    lf_path="$(command -v lf)"
    lf_dir="$(dirname "$lf_path")"
    case "$lf_dir" in
      "$HOME/.lf/bin"|"$HOME/.local/bin")
        echo "$lf_dir"
        return
        ;;
    esac
  fi

  echo "$DEFAULT_INSTALL_DIR"
}

detect_target() {
  os=$(uname -s)
  arch=$(uname -m)
  case "$os" in
    Darwin)
      case "$arch" in
        arm64|aarch64) echo "aarch64-apple-darwin" ;;
        x86_64) echo "x86_64-apple-darwin" ;;
        *) echo "Unsupported architecture: $arch" >&2; exit 1 ;;
      esac ;;
    Linux)
      case "$arch" in
        aarch64) echo "aarch64-unknown-linux-gnu" ;;
        x86_64) echo "x86_64-unknown-linux-gnu" ;;
        *) echo "Unsupported architecture: $arch" >&2; exit 1 ;;
      esac ;;
    *) echo "Unsupported OS: $os" >&2; exit 1 ;;
  esac
}

# The stream `lf install` follows, recorded in the Machine's ~/.lf/config.yaml.
STREAM_CONFIG="$HOME/.lf/config.yaml"

recorded_stream() {
  if [ -f "$STREAM_CONFIG" ]; then
    sed -n 's/^stream:[[:space:]]*\([a-z]*\).*/\1/p' "$STREAM_CONFIG" | head -n 1
  fi
}

record_stream() {
  mkdir -p "$(dirname "$STREAM_CONFIG")"
  if [ -f "$STREAM_CONFIG" ] && grep -q '^stream:' "$STREAM_CONFIG"; then
    sed "s/^stream:.*/stream: $1/" "$STREAM_CONFIG" > "$STREAM_CONFIG.tmp"
    mv "$STREAM_CONFIG.tmp" "$STREAM_CONFIG"
  else
    if [ -s "$STREAM_CONFIG" ] && [ -n "$(tail -c 1 "$STREAM_CONFIG")" ]; then
      echo >> "$STREAM_CONFIG"
    fi
    echo "stream: $1" >> "$STREAM_CONFIG"
  fi
}

main() {
  target=$(detect_target)
  install_dir="$(resolve_install_dir)"
  version=""
  channel=""
  cli_only="${LF_INSTALL_CLI_ONLY:-0}"

  usage() {
    echo "Usage: install.sh [--channel stable|preview] [--version <X>] [--cli-only] [VERSION]" >&2
  }

  while [ $# -gt 0 ]; do
    case "$1" in
      --version)
        if [ $# -lt 2 ]; then
          echo "--version requires a value" >&2
          usage
          exit 1
        fi
        version="$2"
        shift 2
        ;;
      --version=)
        echo "--version requires a value" >&2
        usage
        exit 1
        ;;
      --version=*) version="${1#--version=}"; shift ;;
      --channel)
        if [ $# -lt 2 ]; then
          echo "--channel requires a value" >&2
          usage
          exit 1
        fi
        channel="$2"
        shift 2
        ;;
      --channel=*) channel="${1#--channel=}"; shift ;;
      --cli-only) cli_only="1"; shift ;;
      -*) echo "Unknown option: $1" >&2; usage; exit 1 ;;
      *) version="$1"; shift ;;
    esac
  done

  if [ -n "$version" ] && [ -n "$channel" ]; then
    echo "Pass --channel or --version, not both" >&2
    usage
    exit 1
  fi
  stream="$channel"
  if [ -z "$version" ] && [ -z "$stream" ]; then
    stream="$(recorded_stream)"
    stream="${stream:-stable}"
  fi

  if [ -z "$version" ]; then
    case "$stream" in
      stable)
        # Asset downloads end at a CDN URL that no longer contains the release tag.
        if ! release_url=$(curl -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/$REPO/releases/latest"); then
          echo "Download failed: stable release lookup" >&2
          exit 1
        fi
        case "$release_url" in
          "https://github.com/$REPO/releases/tag/v"*)
            tag="${release_url##*/}"
            ;;
          *)
            echo "Stable release did not resolve to a pinned tag" >&2
            exit 1
            ;;
        esac
        ;;
      preview)
        # The releases list is newest first; prereleases are not a stream.
        if ! releases=$(curl -fsSL "https://api.github.com/repos/$REPO/releases?per_page=20"); then
          echo "Download failed: preview release lookup" >&2
          exit 1
        fi
        tag=$(printf '%s\n' "$releases" | tr ',' '\n' | awk '
          /"tag_name":/ { gsub(/.*"tag_name":[[:space:]]*"|".*/, ""); candidate = $0 }
          /"prerelease":[[:space:]]*false/ { if (candidate != "") { print candidate; exit } }
          /"prerelease":[[:space:]]*true/ { candidate = "" }
        ')
        if [ -z "$tag" ]; then
          echo "No published release found for the preview stream" >&2
          exit 1
        fi
        ;;
      dev)
        echo "dev builds come from CI and need gh auth; install a release, then run \`lf install --dev\`" >&2
        exit 1
        ;;
      *)
        echo "Unknown channel: $stream (choose stable or preview)" >&2
        usage
        exit 1
        ;;
    esac
  else
    case "$version" in
      v*) tag="$version" ;;
      *) tag="v$version" ;;
    esac
  fi
  release_base="https://github.com/$REPO/releases/download/$tag"
  manifest_url="$release_base/SHA256SUMS"

  echo "Installing lf ($target)..."
  mkdir -p "$install_dir"

  tmpdir="$(mktemp -d)"
  mounted="0"
  cleanup() {
    if [ "$mounted" = "1" ]; then
      hdiutil detach "$tmpdir/mount" >/dev/null 2>&1 || true
    fi
    rm -rf "$tmpdir"
  }
  trap cleanup EXIT INT TERM

  manifest="$tmpdir/SHA256SUMS"
  if ! curl -fsSL -o "$manifest" "$manifest_url"; then
    echo "Download failed: $manifest_url" >&2
    echo "No release found for version '$version' ($target)." >&2
    exit 1
  fi

  verify_asset() {
    asset_name="$1"
    asset_path="$2"
    expected=$(awk -v name="$asset_name" '$2 == name || $2 == "*" name { print $1; exit }' "$manifest")
    if [ -z "$expected" ]; then
      echo "SHA256SUMS does not name $asset_name" >&2
      exit 1
    fi
    if command -v shasum >/dev/null 2>&1; then
      actual=$(shasum -a 256 "$asset_path" | awk '{ print $1 }')
    elif command -v sha256sum >/dev/null 2>&1; then
      actual=$(sha256sum "$asset_path" | awk '{ print $1 }')
    else
      echo "Neither shasum nor sha256sum is available" >&2
      exit 1
    fi
    if [ "$actual" != "$expected" ]; then
      echo "Digest mismatch for $asset_name" >&2
      exit 1
    fi
  }

  archive_name="lf-$target.tar.gz"
  url="$release_base/$archive_name"
  tarball="$tmpdir/$archive_name"
  if ! curl -fsSL "$url" -o "$tarball"; then
    echo "Download failed: $url" >&2
    echo "No release found for version '$version' ($target)." >&2
    exit 1
  fi
  verify_asset "$archive_name" "$tarball"
  tar -xzf "$tarball" -C "$tmpdir"

  src="$tmpdir/lf"
  dst="$install_dir/lf"
  chmod +x "$src"
  app_source=""
  if [ "$(uname -s)" = "Darwin" ] && [ "$cli_only" != "1" ]; then
    applications_dir="${LF_APPLICATIONS_DIR:-/Applications}"
    if [ ! -d "$applications_dir" ] || [ ! -w "$applications_dir" ]; then
      echo "Application target is not writable: $applications_dir (pass --cli-only to skip the app)" >&2
      exit 1
    fi
    dmg_name="Loopflow.dmg"
    dmg="$tmpdir/$dmg_name"
    if ! curl -fsSL "$release_base/$dmg_name" -o "$dmg"; then
      echo "Download failed: $release_base/$dmg_name" >&2
      exit 1
    fi
    verify_asset "$dmg_name" "$dmg"
    hdiutil verify "$dmg" >/dev/null
    mkdir "$tmpdir/mount"
    hdiutil attach "$dmg" -nobrowse -readonly -mountpoint "$tmpdir/mount" >/dev/null
    mounted="1"
    app_source="$tmpdir/mount/Loopflow.app"
    if [ ! -d "$app_source" ]; then
      echo "Loopflow.dmg does not contain Loopflow.app" >&2
      exit 1
    fi
    codesign --verify --deep --strict "$app_source"
    spctl --assess --type execute "$app_source"
    "$src" install promote \
      --cli-target "$dst" \
      --app-source "$app_source" \
      --app-target "$applications_dir/Loopflow.app" \
      --legacy-app-target "$applications_dir/Concerto.app" \
      --sync-skills
  else
    "$src" install promote \
      --cli-target "$dst" \
      --sync-skills
  fi

  if [ -n "$channel" ]; then
    record_stream "$channel"
    echo "Following $channel (recorded in $STREAM_CONFIG)"
  fi
  echo "Installed to $install_dir/lf"

  case ":$PATH:" in
    *":$install_dir:"*) ;;
    *) echo "Add to PATH: export PATH=\"$install_dir:\$PATH\"" ;;
  esac
}

main "$@"
