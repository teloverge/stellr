#!/usr/bin/env bash
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
binary="$repo/target/release/stellr"
port="${STELLR_PORT:-8787}"
url_log="$repo/target/stellr-tailnet-url.txt"

if ! command -v tailscale >/dev/null 2>&1; then
  printf 'Tailscale is not installed or is not available on PATH.\n' >&2
  exit 1
fi

if [[ ! "$port" =~ ^[1-9][0-9]{0,4}$ ]] || (( port > 65535 )); then
  printf 'STELLR_PORT must be an integer from 1 through 65535.\n' >&2
  exit 1
fi

tailnet_ip="$(tailscale ip -4 | sed -n '1p')"
if [[ -z "$tailnet_ip" ]]; then
  printf 'Tailscale did not report a Tailnet IPv4 address. Is it connected?\n' >&2
  exit 1
fi

if [[ ! -x "$binary" ]]; then
  printf 'The release server is missing. Build it with:\n' >&2
  printf '  cargo build --release -p stellr-app --bin stellr --locked\n' >&2
  exit 1
fi

stop_existing_server() {
  local process pid executable argument index
  local -a arguments
  local has_serve address_matches

  for process in /proc/[0-9]*; do
    pid="${process##*/}"
    executable="$(readlink "$process/exe" 2>/dev/null || true)"

    arguments=()
    while IFS= read -r -d '' argument; do
      arguments+=("$argument")
    done < "$process/cmdline" 2>/dev/null || continue
    [[ "$executable" == "$binary" \
      || "$executable" == "$binary (deleted)" \
      || "${arguments[0]:-}" == "$binary" ]] || continue

    has_serve=false
    address_matches=false
    for (( index = 0; index + 1 < ${#arguments[@]}; index++ )); do
      [[ "${arguments[$index]}" == 'serve' ]] && has_serve=true
      if [[ "${arguments[$index]}" == '--addr' \
        && "${arguments[$((index + 1))]}" == "$tailnet_ip:$port" ]]; then
        address_matches=true
      fi
    done

    [[ "$has_serve" == true && "$address_matches" == true ]] || continue
    printf 'Stopping existing Stellr Tailnet server (PID %s).\n' "$pid"
    kill -TERM "$pid"
    for _ in {1..50}; do
      kill -0 "$pid" 2>/dev/null || break
      sleep 0.1
    done
    if kill -0 "$pid" 2>/dev/null; then
      printf 'The existing Stellr server did not stop within five seconds.\n' >&2
      exit 1
    fi
  done
}

stop_existing_server

session_token=''
if [[ -s "$url_log" ]]; then
  previous_url="$(tr -d '\r\n' < "$url_log")"
  expected_prefix="http://$tailnet_ip:$port/?token="
  candidate_token="${previous_url#"$expected_prefix"}"
  if [[ "$previous_url" == "$expected_prefix$candidate_token" \
    && "$candidate_token" =~ ^[0-9a-f]{32}$ ]]; then
    session_token="$candidate_token"
  fi
fi

printf 'Serving Stellr on the Tailnet at http://%s:%s/\n' "$tailnet_ip" "$port"
mkdir -p "$repo/target"
umask 077
printf 'The complete authenticated URL will also be saved to %s\n' "$url_log"
if [[ -n "$session_token" ]]; then
  export STELLR_SESSION_TOKEN="$session_token"
  printf 'Reusing the existing Tailnet session so open browser tabs can reconnect.\n'
fi
exec "$binary" serve --addr "$tailnet_ip:$port" > >(
  while IFS= read -r line; do
    if [[ "$line" == 'stellr cockpit: '* ]]; then
      url="${line#stellr cockpit: }"
      printf '%s\n' "$url" > "$url_log"
      printf 'Open Stellr from any Tailnet device:\n%s\n' "$url"
    else
      printf '%s\n' "$line"
    fi
  done
)
