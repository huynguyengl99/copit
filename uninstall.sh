#!/usr/bin/env bash
# copit uninstaller
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/huynguyengl99/copit/main/uninstall.sh | bash
#
# Environment variables:
#   INSTALL_DIR - Installation directory (default: ~/.local/bin)

set -euo pipefail

BINARY_NAME="copit"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

info() { printf '\033[1;34m%s\033[0m\n' "$*"; }
error() { printf '\033[1;31merror: %s\033[0m\n' "$*" >&2; exit 1; }

main() {
    local binary_path="${INSTALL_DIR}/${BINARY_NAME}"

    if [ ! -f "$binary_path" ]; then
        error "copit not found at ${binary_path}. Set INSTALL_DIR if installed elsewhere."
    fi

    rm "$binary_path"
    info "Removed ${binary_path}"
    info "copit has been uninstalled."
}

main
