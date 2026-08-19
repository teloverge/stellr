#!/usr/bin/env bash
set -euo pipefail

host="${1:-amd-halo}"
user="${STELLR_SSH_USER:-pfdev}"
remote_file='~/dev/stellr/target/stellr-tailnet-url.txt'

if ! command -v ssh >/dev/null 2>&1; then
  printf 'OpenSSH is required but ssh was not found on PATH.\n' >&2
  exit 1
fi

if ! url="$(ssh "$user@$host" "cat $remote_file")"; then
  printf 'Could not retrieve the Stellr URL from %s. Verify SSH access for %s@%s.\n' \
    "$host" "$user" "$host" >&2
  exit 1
fi
url="${url//$'\r'/}"
url="${url//$'\n'/}"

if [[ "$url" != http://* && "$url" != https://* ]]; then
  printf 'The Stellr URL returned by %s is not an HTTP(S) URL.\n' "$host" >&2
  exit 1
fi
if [[ "$url" != *'?token='* && "$url" != *'&token='* ]]; then
  printf 'The Stellr URL returned by %s does not contain a session token.\n' "$host" >&2
  exit 1
fi

printf '%s\n' "$url"
