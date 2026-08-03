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
  exec xvfb-run -a -s '-screen 0 1280x800x24' dbus-run-session -- bash "$script" "$kind" "$package"
fi

app_pid=''
wm_pid=''
installed_package=''
launch_log="$(mktemp)"
wm_log="$(mktemp)"

cleanup() {
  if [[ -n "$app_pid" ]] && kill -0 "$app_pid" 2>/dev/null; then
    kill -TERM "$app_pid" 2>/dev/null || true
    wait "$app_pid" 2>/dev/null || true
  fi
  if [[ -n "$wm_pid" ]] && kill -0 "$wm_pid" 2>/dev/null; then
    kill -TERM "$wm_pid" 2>/dev/null || true
    wait "$wm_pid" 2>/dev/null || true
  fi
  if [[ -n "$installed_package" ]]; then
    sudo apt-get remove -y "$installed_package" >/dev/null 2>&1 || true
  fi
  rm -f "$launch_log" "$wm_log"
}
trap cleanup EXIT

dump_diagnostics() {
  printf '\nApplication process tree:\n' >&2
  ps -eo pid=,ppid=,stat=,comm=,args= --forest >&2 || true
  printf '\nVisible windows:\n' >&2
  window_ids="$(xdotool search --onlyvisible --name '.*' 2>/dev/null || true)"
  if [[ -z "$window_ids" ]]; then
    printf '(none)\n' >&2
  else
    for window_id in $window_ids; do
      window_title="$(xdotool getwindowname "$window_id" 2>/dev/null || printf '<unavailable>')"
      window_pid="$(xdotool getwindowpid "$window_id" 2>/dev/null || printf '<unavailable>')"
      printf 'id=%s pid=%s title=%s\n' "$window_id" "$window_pid" "$window_title" >&2
    done
  fi
  printf '\nListening sockets:\n' >&2
  ss -ltnp >&2 || true
  printf '\nApplication log:\n' >&2
  cat "$launch_log" >&2
  printf '\nWindow-manager log:\n' >&2
  cat "$wm_log" >&2
}

openbox >"$wm_log" 2>&1 &
wm_pid=$!
sleep 0.5
if ! kill -0 "$wm_pid" 2>/dev/null; then
  printf 'The clean-runner window manager failed to start.\n' >&2
  cat "$wm_log" >&2
  exit 1
fi

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

STELLR_STARTUP_DIAGNOSTICS=1 "${command[@]}" >"$launch_log" 2>&1 &
app_pid=$!
visible='false'
listeners=''

for _ in {1..60}; do
  if ! kill -0 "$app_pid" 2>/dev/null; then
    printf 'Packaged Stellr exited before its native shell became ready.\n' >&2
    dump_diagnostics
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
  dump_diagnostics
  exit 1
fi
if [[ -z "$listeners" ]]; then
  printf 'No embedded-server listener was owned by the Stellr process.\n' >&2
  dump_diagnostics
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
