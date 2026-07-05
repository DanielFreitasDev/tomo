#!/usr/bin/env bash
# One-time system prerequisites for building Tomo on Debian/Ubuntu.
# Everything else (Rust crates, npm packages) is fetched by cargo/pnpm.
set -euo pipefail

echo "Installing Tauri 2 system dependencies (requires sudo)…"
sudo apt-get update
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev \
  build-essential \
  curl wget file \
  libxdo-dev \
  libssl-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev

if ! command -v cargo >/dev/null 2>&1; then
  echo "Installing the Rust toolchain…"
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  # shellcheck disable=SC1091
  source "$HOME/.cargo/env"
  rustup component add clippy rustfmt
fi

echo
echo "Done. Now:  pnpm install && pnpm tauri dev"
