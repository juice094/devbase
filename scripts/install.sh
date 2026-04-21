#!/usr/bin/env bash
# devbase Quick Install Script (Linux/macOS)
# Usage: curl -fsSL https://raw.githubusercontent.com/juice094/devbase/main/scripts/install.sh | bash

set -e

REPO_URL="https://github.com/juice094/devbase.git"
INSTALL_DIR="${HOME}/.devbase/src"
BIN_DIR="${HOME}/.devbase/bin"

info() { echo -e "\033[36m[devbase]\033[0m $*"; }
ok()   { echo -e "\033[32m[devbase]\033[0m $*"; }
warn() { echo -e "\033[33m[devbase]\033[0m $*"; }

# 1. Check Rust / cargo
if ! command -v cargo &>/dev/null; then
    warn "Rust (cargo) not found."
    echo "Please install Rust first: https://rustup.rs/"
    exit 1
fi
ok "Found cargo at $(command -v cargo)"

# 2. Clone or update source
if [ -d "${INSTALL_DIR}/.git" ]; then
    info "Updating existing source..."
    git -C "${INSTALL_DIR}" pull --quiet
else
    info "Cloning devbase repository..."
    mkdir -p "${INSTALL_DIR}"
    git clone --depth 1 "${REPO_URL}" "${INSTALL_DIR}" --quiet
fi

# 3. Build release binary
info "Building devbase (release mode)..."
cd "${INSTALL_DIR}"
cargo build --release

# 4. Install binary to bin dir
mkdir -p "${BIN_DIR}"
cp "${INSTALL_DIR}/target/release/devbase" "${BIN_DIR}/devbase"
chmod +x "${BIN_DIR}/devbase"
ok "Installed binary to ${BIN_DIR}/devbase"

# 5. Add to PATH if not present
SHELL_RC=""
case "${SHELL}" in
    */bash) SHELL_RC="${HOME}/.bashrc" ;;
    */zsh)  SHELL_RC="${HOME}/.zshrc" ;;
    */fish) SHELL_RC="${HOME}/.config/fish/config.fish" ;;
    *)      SHELL_RC="${HOME}/.profile" ;;
esac

if [ -n "${SHELL_RC}" ] && [ -f "${SHELL_RC}" ]; then
    if ! grep -q "${BIN_DIR}" "${SHELL_RC}" 2>/dev/null; then
        echo "export PATH=\"${BIN_DIR}:\$PATH\"" >> "${SHELL_RC}"
        ok "Added ${BIN_DIR} to PATH in ${SHELL_RC}"
        warn "Please run: source ${SHELL_RC}"
    else
        ok "bin directory already in PATH"
    fi
fi

# 6. Verify
"${BIN_DIR}/devbase" --version
ok "devbase installation complete!"
echo ""
echo "Quick start:"
echo "  devbase scan .          # scan for repos"
echo "  devbase tui             # launch TUI"
echo "  devbase mcp             # start MCP server (stdio)"
