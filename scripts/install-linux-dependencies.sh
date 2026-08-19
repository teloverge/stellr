#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" != 'Linux' ]]; then
  printf 'Linux development dependencies must be installed on native Linux.\n' >&2
  exit 1
fi

if ! command -v apt-get >/dev/null 2>&1; then
  printf 'This installer supports Debian and Ubuntu through apt-get.\n' >&2
  printf 'Install the equivalent Tauri 2 WebKitGTK 4.1 development packages for your distribution.\n' >&2
  exit 1
fi

apt=(apt-get)
if (( EUID != 0 )); then
  if ! command -v sudo >/dev/null 2>&1; then
    printf 'Run this script as root or install sudo first.\n' >&2
    exit 1
  fi
  apt=(sudo apt-get)
fi

"${apt[@]}" update
"${apt[@]}" install -y \
  build-essential \
  curl \
  file \
  libayatana-appindicator3-dev \
  libdbus-1-dev \
  librsvg2-dev \
  libssl-dev \
  libwebkit2gtk-4.1-dev \
  libxdo-dev \
  patchelf \
  pkg-config \
  wget

printf 'LINUX_DEVELOPMENT_DEPENDENCIES_INSTALLED=true\n'
