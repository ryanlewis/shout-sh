#!/usr/bin/env bash
# Smoke-test the Worker through `wrangler dev`: the parts the host tests
# cannot reach (Static Assets, streaming, HEAD). Needs web/dist from
# `just web-build` and shout-worker/node_modules from `pnpm install`.
set -euo pipefail

cd "$(dirname "$0")/../shout-worker"
PORT="${PORT:-8797}"
BASE="http://127.0.0.1:${PORT}"
LOG="$(mktemp)"
TMP="$(mktemp -d)"

# A fresh state directory: wrangler dev keeps rate-limit counters on disk,
# so a run within a minute of the last one would start over the limit.
./node_modules/.bin/wrangler dev --ip 127.0.0.1 --port "$PORT" --persist-to "$TMP/state" >"$LOG" 2>&1 &
WRANGLER=$!
trap 'kill "$WRANGLER" 2>/dev/null || true; rm -rf "$LOG" "$TMP"' EXIT

for _ in $(seq 1 180); do
	curl -fs -o /dev/null "$BASE/health" && break
	if ! kill -0 "$WRANGLER" 2>/dev/null; then
		cat "$LOG"
		exit 1
	fi
	sleep 1
done

fail=0
check() {
	local name=$1 got=$2 want=$3
	if [[ "$got" == "$want" ]]; then
		echo "ok    $name"
	else
		echo "FAIL  $name: got '$got', want '$want'"
		fail=1
	fi
}
# Print one response header, without the trailing CR.
header() {
	curl -s -D - -o /dev/null "${@:2}" | tr -d '\r' | awk -v h="$1" 'tolower($0) ~ "^"h":" { sub(/^[^:]*: /, ""); print }'
}

check "/health" "$(curl -s "$BASE/health")" "ok"
check "/ plain help" "$(curl -s "$BASE/" | grep -c USAGE)" "1"

check "/ html type" "$(header content-type -H 'Accept: text/html' "$BASE/")" "text/html; charset=utf-8"
check "/ html cache" "$(header cache-control -H 'Accept: text/html' "$BASE/")" "no-cache"
curl -s -H 'Accept: text/html' "$BASE/" >"$TMP/index.html"
check "/ html is the playground" "$(grep -c '<title>shout.sh' "$TMP/index.html")" "1"
check "/about html" "$(header content-type -A 'Mozilla/5.0' "$BASE/about")" "text/html; charset=utf-8"
check "/privacy html" "$(header content-type -H 'Accept: text/html' "$BASE/privacy")" "text/html; charset=utf-8"
check "/about for curl" "$(header content-type "$BASE/about")" "text/plain; charset=utf-8"

# Every /_app/ URL the page references must be served, immutable.
for url in $(grep -o '/_app/[A-Za-z0-9_.-]*' "$TMP/index.html" ../web/dist/_app/*.js | sed 's/^.*:\/_app/\/_app/' | sort -u); do
	check "$url status" "$(curl -s -o /dev/null -w '%{http_code}' "$BASE$url")" "200"
	check "$url cache" "$(header cache-control "$BASE$url")" "public, max-age=31536000, immutable"
done
wasm=$(basename ../web/dist/_app/*.wasm)
check "wasm type" "$(header content-type "$BASE/_app/$wasm")" "application/wasm"
check "wasm magic" "$(curl -s "$BASE/_app/$wasm" | head -c 4 | od -An -c | tr -d ' ')" '\0asm'
check "unknown asset" "$(curl -s -o /dev/null -w '%{http_code}' "$BASE/_app/nope-00000000.js")" "404"

check "/favicon.svg" "$(header cache-control "$BASE/favicon.svg")" "public, max-age=86400"
check "/og.png" "$(header content-type "$BASE/og.png")" "image/png"
check "/favicon.ico" "$(curl -s -o /dev/null -w '%{http_code}' "$BASE/favicon.ico")" "204"

# Streaming: a 1s fire stream at 10fps is a first frame, about 9
# cursor-up redraws, then a reset.
curl -sN "$BASE/fire/boom?timeout=1" >"$TMP/stream"
check "stream starts with hide cursor" "$(head -c 6 "$TMP/stream" | od -An -c | tr -d ' ')" '033[?25l'
check "stream ends with reset" "$(tail -c 11 "$TMP/stream" | od -An -c | tr -d ' \n')" '033[0m033[?25h\n'
redraws=$(grep -ao $'\x1b\\[[0-9]*A\r' "$TMP/stream" | wc -l | tr -d ' ')
check "stream redraws (7-9)" "$((redraws >= 7 && redraws <= 9))" "1"
check "HEAD on stream" "$(curl -s -I --max-time 5 -o /dev/null -w '%{http_code}' "$BASE/rainbow/hi")" "200"
check "POST on named route" "$(curl -s -X POST -o /dev/null -w '%{http_code}' "$BASE/health")" "405"
check "POST on stream" "$(curl -s -X POST --max-time 5 -o /dev/null -w '%{http_code}' "$BASE/fire/boom")" "405"
check "unknown method on stream" "$(curl -s -X PROPFIND --max-time 5 -o /dev/null -w '%{http_code}' "$BASE/fire/boom")" "405"

# Open-stream cap: wrangler dev runs the StreamSlots Durable Object
# locally. Three open streams hold every slot, so a fourth is refused
# until one ends. Uses 5 of the 10 streams STREAM_LIMIT allows a minute.
pids=()
for _ in 1 2 3; do
	curl -s -o /dev/null "$BASE/fire/boom?timeout=3" &
	pids+=($!)
done
sleep 1
curl -s -D "$TMP/slots-headers" -o /dev/null "$BASE/fire/boom?timeout=3"
check "4th open stream refused" "$(awk 'NR == 1 { print $2 }' "$TMP/slots-headers")" "429"
retry=$(tr -d '\r' <"$TMP/slots-headers" | awk 'tolower($1) == "retry-after:" { print $2 }')
check "4th stream retry-after (1-3)" "$((retry >= 1 && retry <= 3))" "1"
wait "${pids[@]}"
check "slot freed when a stream ends" "$(curl -s -o /dev/null -w '%{http_code}' "$BASE/fire/boom?timeout=1")" "200"

# Rate limit: wrangler dev simulates the STREAM_LIMIT binding (10 a
# minute). This runs last because it uses the limit up.
# The open-stream cap refuses a few of the burst as well; the checks
# after the burst only pass because the burst uses up STREAM_LIMIT.
pids=()
for _ in $(seq 1 12); do
	curl -s -o /dev/null -w '%{http_code}\n' "$BASE/fire/boom?timeout=1" >>"$TMP/codes" &
	pids+=($!)
done
# Not a bare `wait`: that would wait for wrangler too.
wait "${pids[@]}"
check "stream limit refuses a burst" "$(grep -c 429 "$TMP/codes" | awk '{ print ($1 > 0) }')" "1"
check "429 status" "$(curl -s -o /dev/null -w '%{http_code}' "$BASE/fire/boom?timeout=1")" "429"
check "429 type" "$(header content-type "$BASE/fire/boom?timeout=1")" "text/plain; charset=utf-8"
check "429 retry-after" "$(header retry-after "$BASE/fire/boom?timeout=1")" "60"
check "health exempt when limited" "$(curl -s "$BASE/health")" "ok"

exit "$fail"
