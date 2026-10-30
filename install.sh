#!/usr/bin/env bash
# Installs the tool for the current user (no root needed).
set -euo pipefail
cd "$(dirname "$0")"
install -Dm755 dlssg-open-proton "$HOME/.local/bin/dlssg-open-proton"
install -Dm644 dlssg-open-proton.desktop "$HOME/.local/share/applications/dlssg-open-proton.desktop"
echo "Installed. Start menu: \"Create dlssg-open Proton\" (German system: \"dlssg-open Proton erstellen\")."
