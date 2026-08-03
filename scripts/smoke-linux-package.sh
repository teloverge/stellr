#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 || ( "$1" != 'AppImage' && "$1" != 'Deb' ) ]]; then
  printf 'Usage: %s <AppImage|Deb> <package-path>\n' "$0" >&2
  exit 2
fi
if [[ "$(uname -s)" != 'Linux' || "$(uname -m)" != 'x86_64' ]]; then
  printf 'Linux package smoke tests require native x86_64 Linux.\n' >&2
  exit 1
fi

kind="$1"
package="$(realpath "$2")"
script="$(realpath "$0")"

if [[ -z "${DISPLAY:-}" ]]; then
  exec xvfb-run -a -s '-screen 0 1280x800x24' bash "$script" "$kind" "$package"
fi

app_pid=''
installed_package=''
launch_log="$(mktemp)"

cleanup() {
  if [[ -n "$app_pid" ]] && kill -0 "$app_pid" 2>/dev/null; then
    kill -TERM "$app_pid" 2>/dev/null || true
    wait "$app_pid" 2>/dev/null || true
  fi
  if [[ -n "$installed_package" ]]; then
    sudo apt-get remove -y "$installed_package" >/dev/null 2>&1 || true
  fi
  rm -f "$launch_log"
}
trap cleanup EXIT

if [[ "$kind" == 'AppImage' ]]; then
  chmod +x "$package"
  command=("$package")
else
  installed_package="$(dpkg-deb --field "$package" Package)"
  if dpkg-query --show "$installed_package" >/dev/null 2>&1; then
    printf '%s is already installed; refusing to overwrite it during a clean-runner smoke test.\n' "$installed_package" >&2
    exit 1
  fi
  sudo apt-get install -y "$package"
  executable="$(command -v stellr)"
  command=("$executable")
fi

"${command[@]}" >"$launch_log" 2>&1 &
app_pid=$!
visible='false'
listeners=''

for _ in {1..60}; do
  if ! kill -0 "$app_pid" 2>/dev/null; then
    printf 'Packaged Stellr exited before its native shell became ready.\n' >&2
    cat "$launch_log" >&2
    exit 1
  fi
  if xdotool search --onlyvisible --name '^Stellr$' >/dev/null 2>&1; then
    visible='true'
    listeners="$(ss -ltnp 2>/dev/null | grep -F "pid=$app_pid," || true)"
    if [[ -n "$listeners" ]]; then
      break
    fi
  fi
  sleep 0.5
done

if [[ "$visible" != 'true' ]]; then
  printf 'The packaged native Stellr window never became visible.\n' >&2
  cat "$launch_log" >&2
  exit 1
fi
if [[ -z "$listeners" ]]; then
  printf 'No embedded-server listener was owned by the Stellr process.\n' >&2
  cat "$launch_log" >&2
  exit 1
fi

while IFS= read -r listener; do
  local_address="$(awk '{print $4}' <<<"$listener")"
  case "$local_address" in
    127.0.0.1:*|'[::1]':*) ;;
    *)
      printf 'Non-loopback listener detected: %s\n' "$listener" >&2
      exit 1
      ;;
  esac
done <<<"$listeners"

kill -TERM "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=''

printf 'LINUX_NATIVE_WINDOW_VISIBLE=true\n'
printf 'LINUX_SERVER_LOOPBACK_ONLY=true\n'
printf 'LINUX_%s_SMOKE_PASSED=true\n' "${kind^^}"
