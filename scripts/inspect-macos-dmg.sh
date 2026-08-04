#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 || ( "$2" != 'Development' && "$2" != 'Release' ) ]]; then
  printf 'Usage: %s <dmg-path> <Development|Release>\n' "$0" >&2
  exit 2
fi
if [[ "$(uname -s)" != 'Darwin' ]]; then
  printf 'DMG inspection must run on native macOS.\n' >&2
  exit 1
fi

dmg="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
channel="$2"
mount_point="$(mktemp -d)"
app_pid=''

cleanup() {
  if [[ -n "$app_pid" ]] && kill -0 "$app_pid" 2>/dev/null; then
    kill -TERM "$app_pid" 2>/dev/null || true
    wait "$app_pid" 2>/dev/null || true
  fi
  hdiutil detach "$mount_point" -quiet 2>/dev/null || true
  rm -rf "$mount_point"
}
trap cleanup EXIT

hdiutil attach -readonly -nobrowse -mountpoint "$mount_point" "$dmg" >/dev/null
app="$(find "$mount_point" -maxdepth 1 -type d -name 'Stellr.app' -print -quit)"
if [[ -z "$app" ]]; then
  printf 'The DMG does not contain Stellr.app.\n' >&2
  exit 1
fi

plist="$app/Contents/Info.plist"
binary="$app/Contents/MacOS/stellr"
plutil -lint "$plist" >/dev/null
if [[ ! -x "$binary" ]]; then
  printf 'The packaged Stellr executable is missing.\n' >&2
  exit 1
fi

architectures="$(lipo -archs "$binary")"
if [[ " $architectures " != *' arm64 '* || " $architectures " != *' x86_64 '* ]]; then
  printf 'Expected arm64 and x86_64 slices, found: %s\n' "$architectures" >&2
  exit 1
fi

signature="$(codesign -dv --verbose=4 "$app" 2>&1 || true)"
if [[ "$channel" == 'Development' ]]; then
  if [[ "$signature" == *'Authority=Developer ID Application'* ]]; then
    printf 'Development DMG unexpectedly carries a Developer ID identity.\n' >&2
    exit 1
  fi
else
  codesign --verify --deep --strict --verbose=2 "$app"
  if [[ "$signature" != *'Authority=Developer ID Application'* ]]; then
    printf 'Signed DMG lacks a Developer ID Application identity.\n' >&2
    exit 1
  fi

  launch_log="$(mktemp)"
  "$binary" >"$launch_log" 2>&1 &
  app_pid=$!
  sleep 8
  if ! kill -0 "$app_pid" 2>/dev/null; then
    printf 'The signed packaged application exited during its launch gate.\n' >&2
    cat "$launch_log" >&2
    exit 1
  fi
  kill -TERM "$app_pid"
  wait "$app_pid" 2>/dev/null || true
  app_pid=''
  rm -f "$launch_log"
fi

printf 'MACOS_APP_ARCHITECTURES=%s\n' "$architectures"
printf 'MACOS_DMG_INSPECTION_PASSED=true\n'
