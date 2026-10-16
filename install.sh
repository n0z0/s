#!/bin/sh
# ==============================================================================
# Installer script for `s` - Ultra-lightweight terminal session manager
# GitHub: https://github.com/n0z0/s
# Website: https://n0z0.if.co.id/s
#
# Usage:
#   curl -fsSL https://n0z0.if.co.id/s/install.sh | sh
#   or
#   wget -qO- https://n0z0.if.co.id/s/install.sh | sh
# ==============================================================================

set -e

REPO="n0z0/s"
APP_NAME="s"

# Color formatting
if [ -t 1 ]; then
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    BLUE='\033[0;34m'
    YELLOW='\033[1;33m'
    BOLD='\033[1m'
    RESET='\033[0m'
else
    RED=''
    GREEN=''
    BLUE=''
    YELLOW=''
    BOLD=''
    RESET=''
fi

info() {
    printf "${BLUE}${BOLD}[*]${RESET} %s\n" "$1"
}

success() {
    printf "${GREEN}${BOLD}[+]${RESET} %s\n" "$1"
}

warn() {
    printf "${YELLOW}${BOLD}[!]${RESET} %s\n" "$1"
}

error() {
    printf "${RED}${BOLD}[x]${RESET} %s\n" "$1" >&2
    exit 1
}

# Print Banner
printf "${BOLD}"
cat << 'EOF'
     ___
    / __|
    \__ \
    |___/   Terminal Session Manager (TMUX/DTACH Alternative)
EOF
printf "${RESET}\n"

# 1. Detect OS
OS="$(uname -s)"
case "$OS" in
    Linux*)     OS_NAME="unknown-linux-musl" ;;
    Darwin*)    OS_NAME="apple-darwin" ;;
    *)          error "Unsupported operating system: $OS. `s` requires Linux or macOS." ;;
esac

# 2. Detect Architecture
ARCH="$(uname -m)"
case "$ARCH" in
    x86_64|amd64)   ARCH_NAME="x86_64" ;;
    aarch64|arm64)  ARCH_NAME="aarch64" ;;
    *)              error "Unsupported architecture: $ARCH." ;;
esac

TARGET="${ARCH_NAME}-${OS_NAME}"
info "Detected platform: ${BOLD}${TARGET}${RESET}"

# 3. Determine Installation Directory
if [ -n "$INSTALL_DIR" ]; then
    BIN_DIR="$INSTALL_DIR"
elif [ "$(id -u)" -eq 0 ]; then
    BIN_DIR="/usr/local/bin"
elif [ -d "$HOME/.local/bin" ] || [ -w "$HOME/.local" ]; then
    BIN_DIR="$HOME/.local/bin"
else
    BIN_DIR="/usr/local/bin"
fi

# Ensure bin directory exists
if [ ! -d "$BIN_DIR" ]; then
    mkdir -p "$BIN_DIR" 2>/dev/null || {
        # Fallback to ~/.local/bin
        BIN_DIR="$HOME/.local/bin"
        mkdir -p "$BIN_DIR"
    }
fi

info "Installation destination: ${BOLD}${BIN_DIR}/${APP_NAME}${RESET}"

# 4. Find Latest Release Version
info "Fetching latest release information from GitHub (${REPO})..."

if command -v curl >/dev/null 2>&1; then
    LATEST_JSON=$(curl -sSL "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null || true)
elif command -v wget >/dev/null 2>&1; then
    LATEST_JSON=$(wget -qO- "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null || true)
else
    error "Either 'curl' or 'wget' is required to download $APP_NAME."
fi

# Parse tag_name
VERSION=$(printf '%s\n' "$LATEST_JSON" | grep '"tag_name":' | head -n 1 | sed -E 's/.*"tag_name": *"([^"]+)".*/\1/')

if [ -z "$VERSION" ]; then
    warn "Could not resolve latest release via GitHub API. Falling back to 'v0.1.0'..."
    VERSION="v0.1.0"
fi

TARBALL="s-${TARGET}.tar.gz"
DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${VERSION}/${TARBALL}"

info "Downloading ${BOLD}${TARBALL}${RESET} (${VERSION})..."

# 5. Create temporary working directory
TMP_DIR="$(mktemp -d 2>/dev/null || mktemp -d -t 's-install')"
cleanup() {
    rm -rf "$TMP_DIR"
}
trap cleanup EXIT INT TERM

# 6. Download release archive
if command -v curl >/dev/null 2>&1; then
    if ! curl -f -L -o "${TMP_DIR}/${TARBALL}" "$DOWNLOAD_URL"; then
        # Try fallback direct binary download
        FALLBACK_URL="https://github.com/${REPO}/releases/download/${VERSION}/s-${TARGET}"
        info "Trying direct binary: ${FALLBACK_URL}..."
        curl -f -L -o "${TMP_DIR}/${APP_NAME}" "$FALLBACK_URL" || error "Failed to download $APP_NAME from $DOWNLOAD_URL"
    fi
elif command -v wget >/dev/null 2>&1; then
    if ! wget -q -O "${TMP_DIR}/${TARBALL}" "$DOWNLOAD_URL"; then
        FALLBACK_URL="https://github.com/${REPO}/releases/download/${VERSION}/s-${TARGET}"
        info "Trying direct binary: ${FALLBACK_URL}..."
        wget -q -O "${TMP_DIR}/${APP_NAME}" "$FALLBACK_URL" || error "Failed to download $APP_NAME from $DOWNLOAD_URL"
    fi
fi

# 7. Extract archive if tarball was downloaded
if [ -f "${TMP_DIR}/${TARBALL}" ]; then
    tar -xzf "${TMP_DIR}/${TARBALL}" -C "$TMP_DIR"
fi

if [ ! -f "${TMP_DIR}/${APP_NAME}" ]; then
    error "Downloaded package did not contain executable '${APP_NAME}'."
fi

# 8. Install executable
chmod +x "${TMP_DIR}/${APP_NAME}"

if [ -w "$BIN_DIR" ]; then
    mv "${TMP_DIR}/${APP_NAME}" "${BIN_DIR}/${APP_NAME}"
else
    info "Elevated permissions required to write to ${BIN_DIR}. Prompting sudo..."
    sudo mv "${TMP_DIR}/${APP_NAME}" "${BIN_DIR}/${APP_NAME}"
fi

# 9. Verify Installation
if [ -x "${BIN_DIR}/${APP_NAME}" ]; then
    success "Successfully installed ${BOLD}${APP_NAME}${RESET} to ${BOLD}${BIN_DIR}/${APP_NAME}${RESET}"
else
    error "Installation verification failed."
fi

# 10. Check PATH
case ":$PATH:" in
    *":${BIN_DIR}:"*)
        PATH_OK=true
        ;;
    *)
        PATH_OK=false
        ;;
esac

if [ "$PATH_OK" = "false" ]; then
    printf "\n"
    warn "${BIN_DIR} is not in your current PATH!"
    printf "  To use '${BOLD}s${RESET}' from anywhere, add the following to your ${BOLD}~/.bashrc${RESET} or ${BOLD}~/.zshrc${RESET}:\n\n"
    printf "    ${GREEN}export PATH=\"%s:\$PATH\"${RESET}\n\n" "$BIN_DIR"
fi

printf "\n${GREEN}${BOLD}🚀 Sesi Pertama Anda:${RESET}\n"
printf "  s main              # Membuat atau masuk ke sesi 'main'\n"
printf "  s list              # Melihat daftar sesi aktif\n"
printf "  Ctrl + \\            # Melepas sesi (detach)\n"
printf "  tail -f /tmp/s_main.log # Mengintip log riwayat sesi\n\n"
