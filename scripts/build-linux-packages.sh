#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" != 'Linux' ]]; then
  printf 'Linux packages must be built on native Linux.\n' >&2
  exit 1
fi
if [[ "$(uname -m)" != 'x86_64' ]]; then
  printf 'The supported Linux packages require an x86_64 runner.\n' >&2
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
  "$cli" build --bundles appimage,deb
)

appimage="$(find "$repo/target/release/bundle/appimage" -maxdepth 1 -type f -name '*.AppImage' -print -quit)"
deb="$(find "$repo/target/release/bundle/deb" -maxdepth 1 -type f -name '*.deb' -print -quit)"
if [[ -z "$appimage" || -z "$deb" ]]; then
  printf 'Tauri did not produce both AppImage and deb packages.\n' >&2
  exit 1
fi

artifact_dir="$repo/artifacts/linux-x86_64"
mkdir -p "$artifact_dir"
appimage_artifact="$artifact_dir/Stellr_${version}_linux-x86_64_appimage_UNSIGNED-NOT-FOR-RELEASE.AppImage"
deb_artifact="$artifact_dir/Stellr_${version}_linux-x86_64_deb_UNSIGNED-NOT-FOR-RELEASE.deb"
cp "$appimage" "$appimage_artifact"
cp "$deb" "$deb_artifact"
chmod +x "$appimage_artifact"
(
  cd "$artifact_dir"
  sha256sum "$(basename "$appimage_artifact")" > "$(basename "$appimage_artifact").sha256"
  sha256sum "$(basename "$deb_artifact")" > "$(basename "$deb_artifact").sha256"
)

printf 'LINUX_APPIMAGE_ARTIFACT=%s\n' "$appimage_artifact"
printf 'LINUX_DEB_ARTIFACT=%s\n' "$deb_artifact"
