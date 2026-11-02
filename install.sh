#!/usr/bin/env bash
# Installs the tool for the current user (no root needed).
# Builds from source when Rust (cargo) is installed, otherwise downloads the prebuilt binary of the latest release.
set -euo pipefail
cd "$(dirname "$0")"
bin="$HOME/.local/bin/dlssg-open-proton"
mkdir -p "$(dirname "$bin")"
if command -v cargo >/dev/null; then
    cargo build --release
    install -m755 "${CARGO_TARGET_DIR:-target}/release/dlssg-open-proton" "$bin"
else
    curl -fsSL -o "$bin.part" https://github.com/ProfessorTee/dlssg-open-proton/releases/latest/download/dlssg-open-proton
    chmod 755 "$bin.part" && mv "$bin.part" "$bin"
fi
install -Dm644 dlssg-open-proton.desktop "$HOME/.local/share/applications/dlssg-open-proton.desktop"
echo "Installed. Start menu: \"Create dlssg-open Proton\" (German system: \"dlssg-open Proton erstellen\")."
