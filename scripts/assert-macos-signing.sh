#!/usr/bin/env bash
set -euo pipefail

missing=()
for variable in APPLE_CERTIFICATE APPLE_CERTIFICATE_PASSWORD KEYCHAIN_PASSWORD; do
  if [[ -z "${!variable:-}" ]]; then
    missing+=("$variable")
  fi
done

if (( ${#missing[@]} > 0 )); then
  printf 'Official tagged macOS releases require: %s\n' "${missing[*]}" >&2
  exit 1
fi

if ! printf '%s' "$APPLE_CERTIFICATE" | openssl base64 -d -A >/dev/null 2>&1; then
  printf 'APPLE_CERTIFICATE is not valid base64.\n' >&2
  exit 1
fi

printf 'MACOS_SIGNING_PREFLIGHT_PASSED=true\n'
