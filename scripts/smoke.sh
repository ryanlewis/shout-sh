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
# Next to wrangler.toml, so its relative paths (../web/dist, build/)
# still resolve. Named per run, so two runs in one checkout do not
# overwrite or delete each other's copy.
CONFIG="wrangler.smoke.$$.toml"
WRANGLER=
trap 'if [[ -n "$WRANGLER" ]]; then kill "$WRANGLER" 2>/dev/null || true; fi; rm -rf "$LOG" "$TMP" "$CONFIG"' EXIT

# The `limit` of one [[ratelimits]] binding in a wrangler config.
limit_of() {
	awk -v want="\"$2\"" '/^\[/ { name = "" } $1 == "name" { name = $3 } $1 == "simple" && name == want { match($0, /limit = [0-9]+/); print substr($0, RSTART + 8, RLENGTH - 8) }' "$1"
}

# The production limits are too low for one run of this script, so it
# runs on a copy with other limits: RATE_LIMIT 1000, so the general
# requests never reach it, and STREAM_LIMIT 10, which the budget below
# is written against. The copy is made from wrangler.toml on every run,
# and the production values are checked in wrangler.toml itself, so the
# copy cannot hide a wrong edit there.
awk '/^\[/ { name = "" }
	$1 == "name" { name = $3 }
	$1 == "simple" && name == "\"RATE_LIMIT\"" { sub(/limit = [0-9]+/, "limit = 1000") }
	$1 == "simple" && name == "\"STREAM_LIMIT\"" { sub(/limit = [0-9]+/, "limit = 10") }
	{ print }' wrangler.toml >"$CONFIG"
prod="$(limit_of wrangler.toml RATE_LIMIT) $(limit_of wrangler.toml STREAM_LIMIT)"
smoke="$(limit_of "$CONFIG" RATE_LIMIT) $(limit_of "$CONFIG" STREAM_LIMIT)"
if [[ "$prod" != "30 5" || "$smoke" != "1000 10" ]]; then
	echo "FAIL  limits: wrangler.toml has '$prod', want '30 5'; $CONFIG has '$smoke', want '1000 10'"
	exit 1
fi
echo "ok    wrangler.toml limits 30 and 5"

# A fresh state directory: wrangler dev keeps rate-limit counters on disk,
# so a run within a minute of the last one would start over the limit.
# A throwaway SLOT_KEY_SECRET, so the open-stream cap runs (see below).
# It overrides any value in .dev.vars.
./node_modules/.bin/wrangler dev -c "$CONFIG" --ip 127.0.0.1 --port "$PORT" --persist-to "$TMP/state" \
	--var "SLOT_KEY_SECRET:smoke-$RANDOM$RANDOM" >"$LOG" 2>&1 &
WRANGLER=$!

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
# Poll until `test` (a command and its arguments) succeeds, for at most
# $1 tenths of a second. sleep 0.1 works on Linux and macOS.
wait_until() {
	local tries=$1 i
	shift
	for ((i = 0; i < tries; i++)); do
		"$@" && return 0
		sleep 0.1
	done
	return 1
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
# A large banner is capped, and HEAD shows the cap without streaming.
# 200 wide 3d letters is the worst case the parser accepts; see
# shout-worker/tests/stream.rs. HEAD never streams and does not use up
# STREAM_LIMIT.
big="$BASE/rainbow+3d/$(printf 'W%.0s' $(seq 1 200))?ls=10&pad=10&fps=30&timeout=300"
check "large banner X-Shout-Capped" "$(header x-shout-capped -I --max-time 5 "$big")" "fps=1; timeout=46"
check "small stream has no X-Shout-Capped" "$(header x-shout-capped -I --max-time 5 "$BASE/fire/boom")" ""
check "HEAD on stream" "$(curl -s -I --max-time 5 -o /dev/null -w '%{http_code}' "$BASE/rainbow/hi")" "200"
check "POST on named route" "$(curl -s -X POST -o /dev/null -w '%{http_code}' "$BASE/health")" "405"
check "POST on stream" "$(curl -s -X POST --max-time 5 -o /dev/null -w '%{http_code}' "$BASE/fire/boom")" "405"
check "unknown method on stream" "$(curl -s -X PROPFIND --max-time 5 -o /dev/null -w '%{http_code}' "$BASE/fire/boom")" "405"

# Open-stream cap: wrangler dev runs the StreamSlots Durable Object
# locally. Three open streams hold every slot, so a fourth is refused
# until one ends. STREAM_LIMIT allows 10 a minute in $CONFIG (5 in
# production) and counts every GET that would stream, refused or not.
# This script sends the 1s stream above (1), the 3 held streams and the
# refused 4th (4), and the slot-freed request plus up to 5 retries
# (1-6). A passing run uses at most 10 before the burst below, which is
# meant to use the rest up.
pids=()
for i in 1 2 3; do
	curl -s -o "$TMP/slot$i" "$BASE/fire/boom?timeout=3" &
	pids+=($!)
done
# A stream holds its slot once the first frame arrives. Wait for all
# three, at most 10s.
slots_held() { [[ -s "$TMP/slot1" && -s "$TMP/slot2" && -s "$TMP/slot3" ]]; }
started=0
wait_until 100 slots_held && started=1
check "3 streams started within 10s" "$started" "1"
curl -s -D "$TMP/slots-headers" -o /dev/null "$BASE/fire/boom?timeout=3"
check "4th open stream refused" "$(awk 'NR == 1 { print $2 }' "$TMP/slots-headers")" "429"
retry=$(tr -d '\r' <"$TMP/slots-headers" | awk 'tolower($1) == "retry-after:" { print $2 }')
# 1-3s until the oldest stream ends, plus slots::RETRY_SLACK_SECS (2).
check "4th stream retry-after (3-5)" "$((retry >= 3 && retry <= 5))" "1"
wait "${pids[@]}"
# The release runs under waitUntil after the body ends, so it can land
# after curl returns. Retry until a stream is accepted, for at most 3s.
# A refused retry still counts against STREAM_LIMIT; see the count above.
freed=000
for _ in $(seq 1 6); do
	freed=$(curl -s -o /dev/null -w '%{http_code}' "$BASE/fire/boom?timeout=1")
	[[ "$freed" == 200 ]] && break
	sleep 0.5
done
check "slot freed when a stream ends" "$freed" "200"

# Rate limit: wrangler dev simulates the STREAM_LIMIT binding (10 a
# minute in $CONFIG). This runs last because it uses the limit up.
# The open-stream cap refuses a few of the burst as well; the checks
# after the burst only pass because the burst uses up STREAM_LIMIT.
# miniflare's windows are fixed minutes, so start before second 50.
early_in_minute() { (($(date +%-S) < 50)); }
wait_until 110 early_in_minute || true
pids=()
for _ in $(seq 1 12); do
	curl -s -o /dev/null -w '%{http_code} %header{retry-after}\n' "$BASE/fire/boom?timeout=1" >>"$TMP/codes" &
	pids+=($!)
done
# Not a bare `wait`: that would wait for wrangler too.
wait "${pids[@]}"
# Only STREAM_LIMIT answers 429 with retry-after 60; the open-stream cap
# gives a few seconds.
check "stream limit refuses a burst" "$(grep -c '^429 60$' "$TMP/codes" | awk '{ print ($1 > 0) }')" "1"
check "429 status" "$(curl -s -o /dev/null -w '%{http_code}' "$BASE/fire/boom?timeout=1")" "429"
check "429 type" "$(header content-type "$BASE/fire/boom?timeout=1")" "text/plain; charset=utf-8"
check "429 retry-after" "$(header retry-after "$BASE/fire/boom?timeout=1")" "60"
check "health exempt when limited" "$(curl -s "$BASE/health")" "ok"

exit "$fail"
