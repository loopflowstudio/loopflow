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

main() {
  target=$(detect_target)
  install_dir="$(resolve_install_dir)"
  version="latest"

  usage() {
    echo "Usage: install.sh [--version <X>] [VERSION]" >&2
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
      -*) echo "Unknown option: $1" >&2; usage; exit 1 ;;
      *) version="$1"; shift ;;
    esac
  done

  if [ "$version" = "latest" ]; then
    url="https://github.com/$REPO/releases/latest/download/lf-$target.tar.gz"
  else
    url="https://github.com/$REPO/releases/download/v$version/lf-$target.tar.gz"
  fi

  echo "Installing lf ($target)..."
  mkdir -p "$install_dir"

  tmpdir="$(mktemp -d)"
  cleanup() {
    rm -rf "$tmpdir"
  }
  trap cleanup EXIT INT TERM

  tarball="$tmpdir/lf.tar.gz"
  if ! curl -fsSL "$url" -o "$tarball"; then
    echo "Download failed: $url" >&2
    echo "No release found for version '$version' ($target)." >&2
    exit 1
  fi
  tar -xzf "$tarball" -C "$tmpdir"

  src="$tmpdir/lf"
  daemon_src="$tmpdir/lfd"
  dst="$install_dir/lf"
  daemon_dst="$install_dir/lfd"
  chmod +x "$src" "$daemon_src"
  "$src" install promote \
    --cli-target "$dst" \
    --daemon-source "$daemon_src" \
    --daemon-target "$daemon_dst"

  echo "Installed to $install_dir/lf and $install_dir/lfd"

  case ":$PATH:" in
    *":$install_dir:"*) ;;
    *) echo "Add to PATH: export PATH=\"$install_dir:\$PATH\"" ;;
  esac
}

main "$@"
