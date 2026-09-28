#!/usr/bin/env bash
# Compare the live site with a local `wrangler dev`, path by path: status,
# Content-Type, Cache-Control and body, byte for byte. Animated paths are
# cut at the first cursor-up, so only the first frame is compared.
#
#   scripts/parity.sh [live-base] [local-base]
#
# Start `just dev` (or `pnpm --dir shout-worker dev`) first. Only GETs are
# sent to the live site.
set -uo pipefail

LIVE="${1:-https://shout.sh}"
LOCAL="${2:-http://127.0.0.1:8787}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

LONG="/$(printf 'a%.0s' $(seq 1 600))"
HTML='Accept: text/html'
CURL_UA='User-Agent: curl/8.7.1'

# name | path | extra header ("" for curl's defaults) | first-frame-only
CASES=(
	"root|/||"
	"root html|/|$HTML|"
	"health|/health||"
	"favicon.ico|/favicon.ico||"
	"favicon.svg|/favicon.svg||"
	"og.png|/og.png||"
	"HELLO|/HELLO||"
	"tiny|/tiny/hello+world||"
	"red|/red/alert||"
	"sunset|/sunset/hi||"
	"ocean 3d|/ocean/3d/Hello||"
	"fonts|/fonts||"
	"fonts/block|/fonts/block||"
	"fonts unknown|/fonts/standard||"
	"font query unknown|/Hi?font=standard||"
	"presets|/presets||"
	"presets/sunset|/presets/sunset||"
	"json|/HELLO?format=json||"
	"rainbow once|/rainbow+once/hi||"
	"too long|$LONG||"
	"about curl|/about|$CURL_UA|"
	"about html|/about|$HTML|"
	"privacy curl|/privacy|$CURL_UA|"
	"privacy html|/privacy|$HTML|"
	"fire first frame|/fire/boom?timeout=1||first"
	"rainbow first frame|/rainbow/hi?timeout=1||first"
)

fetch() { # base path header first out
	local args=(-s -D "$5.h" -o "$5.b" --max-time 20)
	[[ -n "$3" ]] && args+=(-H "$3")
	curl "${args[@]}" "$1$2"
	if [[ -n "$4" ]]; then
		# Keep bytes up to the first ESC[<n>A (cursor-up).
		perl -0777 -i -pe 's/\e\[\d+A.*//s' "$5.b"
	fi
	tr -d '\r' <"$5.h" | awk '
		NR == 1 { print $2 }
		tolower($1) == "content-type:" || tolower($1) == "cache-control:" { print tolower($1), substr($0, index($0, $2)) }
	' | sort >"$5.meta"
}

same=0
diff=0
for c in "${CASES[@]}"; do
	IFS='|' read -r name path header first <<<"$c"
	fetch "$LIVE" "$path" "$header" "$first" "$TMP/live"
	fetch "$LOCAL" "$path" "$header" "$first" "$TMP/local"
	problems=()
	cmp -s "$TMP/live.meta" "$TMP/local.meta" || problems+=("status/headers")
	cmp -s "$TMP/live.b" "$TMP/local.b" || problems+=("body")
	if ((${#problems[@]} == 0)); then
		echo "same  $name"
		same=$((same + 1))
	else
		echo "DIFF  $name: ${problems[*]}"
		diff -u --label live --label local "$TMP/live.meta" "$TMP/local.meta" | sed 's/^/      /'
		if [[ " ${problems[*]} " == *" body "* ]]; then
			echo "      body: live $(wc -c <"$TMP/live.b") bytes, local $(wc -c <"$TMP/local.b") bytes"
			diff <(od -An -c "$TMP/live.b") <(od -An -c "$TMP/local.b") | head -6 | sed 's/^/      /'
		fi
		diff=$((diff + 1))
	fi
done
echo "$same same, $diff different"
((diff == 0))
