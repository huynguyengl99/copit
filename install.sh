#!/usr/bin/env bash
# copit installer
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/huynguyengl99/copit/main/install.sh | bash
#
# Environment variables:
#   COPIT_VERSION   - Version to install (default: latest)
#   INSTALL_DIR     - Installation directory (default: ~/.local/bin)

set -euo pipefail

REPO="huynguyengl99/copit"
BINARY_NAME="copit"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

# --- Helpers ---

info() { printf '\033[1;34m%s\033[0m\n' "$*"; }
error() { printf '\033[1;31merror: %s\033[0m\n' "$*" >&2; exit 1; }

detect_platform() {
    local os arch

    os="$(uname -s)"
    arch="$(uname -m)"

    case "$os" in
        Linux)  os="unknown-linux-gnu" ;;
        Darwin) os="apple-darwin" ;;
        *)      error "Unsupported OS: $os" ;;
    esac

    case "$arch" in
        x86_64|amd64)   arch="x86_64" ;;
        aarch64|arm64)  arch="aarch64" ;;
        armv7l)         arch="armv7"; os="unknown-linux-gnueabihf" ;;
        i686)           arch="i686" ;;
        *)              error "Unsupported architecture: $arch" ;;
    esac

    echo "${arch}-${os}"
}

get_latest_version() {
    local url="https://api.github.com/repos/${REPO}/releases/latest"
    if command -v curl &>/dev/null; then
        curl -fsSL "$url" | grep '"tag_name"' | sed -E 's/.*"tag_name": *"([^"]+)".*/\1/'
    elif command -v wget &>/dev/null; then
        wget -qO- "$url" | grep '"tag_name"' | sed -E 's/.*"tag_name": *"([^"]+)".*/\1/'
    else
        error "Neither curl nor wget found. Please install one."
    fi
}

download() {
    local url="$1" dest="$2"
    if command -v curl &>/dev/null; then
        curl -fsSL -o "$dest" "$url"
    elif command -v wget &>/dev/null; then
        wget -qO "$dest" "$url"
    fi
}

# --- Main ---

tmpdir=""

main() {
    local platform version archive_name archive_url

    platform="$(detect_platform)"
    info "Detected platform: ${platform}"

    if [ -n "${COPIT_VERSION:-}" ]; then
        version="$COPIT_VERSION"
    else
        info "Fetching latest version..."
        version="$(get_latest_version)"
    fi

    if [ -z "$version" ]; then
        error "Could not determine latest version. Set COPIT_VERSION manually."
    fi

    info "Installing copit ${version}..."

    archive_name="${BINARY_NAME}-${platform}.tar.gz"
    archive_url="https://github.com/${REPO}/releases/download/${version}/${archive_name}"

    tmpdir="$(mktemp -d)"
    trap 'rm -rf "$tmpdir"' EXIT

    info "Downloading ${archive_url}..."
    download "$archive_url" "${tmpdir}/${archive_name}"

    info "Extracting..."
    tar xzf "${tmpdir}/${archive_name}" -C "$tmpdir"

    mkdir -p "$INSTALL_DIR"
    cp "${tmpdir}/${BINARY_NAME}-${platform}/${BINARY_NAME}" "${INSTALL_DIR}/${BINARY_NAME}"
    chmod +x "${INSTALL_DIR}/${BINARY_NAME}"

    info "Installed copit to ${INSTALL_DIR}/${BINARY_NAME}"

    # Check if INSTALL_DIR is in PATH
    case ":$PATH:" in
        *":${INSTALL_DIR}:"*) ;;
        *)
            echo ""
            info "Add ${INSTALL_DIR} to your PATH:"
            echo ""
            echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
            echo ""
            echo "  Add this to your ~/.bashrc, ~/.zshrc, or shell config."
            ;;
    esac
}

main
