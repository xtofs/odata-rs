#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:3000}"
STARTUP_TIMEOUT_SECONDS="${STARTUP_TIMEOUT_SECONDS:-30}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
HURL_FILE="${REPO_ROOT}/examples/rooms/demo.hurl"

if ! command -v hurl >/dev/null 2>&1; then
  echo "hurl is not installed or not in PATH. Install it from https://hurl.dev/docs/installation.html" >&2
  exit 1
fi

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is not installed or not in PATH." >&2
  exit 1
fi

SERVER_PID=""
cleanup() {
  if [[ -n "${SERVER_PID}" ]] && kill -0 "${SERVER_PID}" 2>/dev/null; then
    kill "${SERVER_PID}" 2>/dev/null || true
    wait "${SERVER_PID}" 2>/dev/null || true
  fi
}
trap cleanup EXIT INT TERM

(
  cd "${REPO_ROOT}"
  cargo run -p odata-rs --example rooms --features sqlx-sqlite
) &
SERVER_PID=$!

deadline=$((SECONDS + STARTUP_TIMEOUT_SECONDS))
ready=0
while (( SECONDS < deadline )); do
  if ! kill -0 "${SERVER_PID}" 2>/dev/null; then
    echo "rooms server exited before becoming ready." >&2
    exit 1
  fi

  if curl -fsS -m 2 "${BASE_URL}/" >/dev/null 2>&1; then
    ready=1
    break
  fi

  sleep 0.5
done

if (( ready == 0 )); then
  echo "rooms server did not become ready within ${STARTUP_TIMEOUT_SECONDS}s at ${BASE_URL}" >&2
  exit 1
fi

hurl --variable "baseUrl=${BASE_URL}" "${HURL_FILE}"
echo "Hurl scenario passed."