#!/usr/bin/env bash
set -euo pipefail

channel="${1:-Development}"
if [[ "$channel" != 'Development' && "$channel" != 'Release' ]]; then
  printf 'Usage: %s [Development|Release]\n' "$0" >&2
  exit 2
fi
if [[ "$(uname -s)" != 'Darwin' ]]; then
  printf 'The universal DMG must be built on native macOS.\n' >&2
  exit 1
fi
if [[ "$channel" == 'Release' && -z "${APPLE_SIGNING_IDENTITY:-}" ]]; then
  printf 'APPLE_SIGNING_IDENTITY is required for an official macOS release build.\n' >&2
  exit 1
fi

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
config="$repo/crates/app/tauri.conf.json"
version="$(node -p "require(process.argv[1]).version" "$config")"
cli="$repo/web/node_modules/.bin/tauri"
if [[ ! -x "$cli" ]]; then
  printf 'The pinned Tauri CLI is missing. Run npm --prefix web ci first.\n' >&2
  exit 1
fi

npm --prefix "$repo/web" run build
(
  cd "$repo/crates/app"
  "$cli" build --features desktop --bundles dmg --target universal-apple-darwin
)

bundle_root="$repo/target/universal-apple-darwin/release/bundle"
dmg="$(find "$bundle_root/dmg" -maxdepth 1 -type f -name '*.dmg' -print -quit)"
if [[ -z "$dmg" ]]; then
  printf 'Tauri did not produce a DMG.\n' >&2
  exit 1
fi

artifact_dir="$repo/artifacts/macos-universal"
mkdir -p "$artifact_dir"
suffix=''
if [[ "$channel" == 'Development' ]]; then
  suffix='_UNSIGNED-NOT-FOR-RELEASE'
fi
artifact="$artifact_dir/Stellr_${version}_macos-universal_dmg${suffix}.dmg"
cp "$dmg" "$artifact"
(
  cd "$artifact_dir"
  shasum -a 256 "$(basename "$artifact")" > "$(basename "$artifact").sha256"
)

printf 'MACOS_DMG_ARTIFACT=%s\n' "$artifact"
