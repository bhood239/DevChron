#!/usr/bin/env bash
# Build DevChron and install it for the current user (no root needed).
#
#   ./scripts/install.sh              build + install binary, launcher entry, icon
#   ./scripts/install.sh --uninstall  remove everything this script installed
#
# Installs to:
#   ~/.local/bin/devchron
#   ~/.local/share/applications/devchron.desktop
#   ~/.local/share/icons/hicolor/scalable/apps/devchron.svg
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

BIN_DIR="${HOME}/.local/bin"
APP_DIR="${HOME}/.local/share/applications"
ICON_DIR="${HOME}/.local/share/icons/hicolor/scalable/apps"

if [[ "${1:-}" == "--uninstall" ]]; then
    rm -fv "${BIN_DIR}/devchron" "${APP_DIR}/devchron.desktop" "${ICON_DIR}/devchron.svg"
    command -v update-desktop-database >/dev/null && update-desktop-database "${APP_DIR}" || true
    echo "Removed. Your config (~/.config/devchron) and stats (~/.local/share/devchron) were left in place."
    exit 0
fi

if ! command -v cargo >/dev/null; then
    echo "error: cargo not found. On Arch:  sudo pacman -S --needed rust   (or use rustup)" >&2
    exit 1
fi

echo "==> Building (release)"
cargo build --release --locked

echo "==> Installing"
install -Dm755 target/release/devchron "${BIN_DIR}/devchron"
install -Dm644 assets/devchron.svg "${ICON_DIR}/devchron.svg"
mkdir -p "${APP_DIR}"
sed "s|@BIN@|${BIN_DIR}/devchron|" assets/devchron.desktop > "${APP_DIR}/devchron.desktop"
chmod 644 "${APP_DIR}/devchron.desktop"
command -v update-desktop-database >/dev/null && update-desktop-database "${APP_DIR}" || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q -t "${HOME}/.local/share/icons/hicolor" 2>/dev/null || true

echo
echo "Installed: ${BIN_DIR}/devchron"
case ":${PATH}:" in
    *":${BIN_DIR}:"*) ;;
    *) echo "note: ${BIN_DIR} is not on your PATH; add it in ~/.zshenv:  export PATH=\"\$HOME/.local/bin:\$PATH\"" ;;
esac
echo "Run it with 'devchron' in a terminal, or search for 'DevChron' in your app launcher."
