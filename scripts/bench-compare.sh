#!/usr/bin/env bash
# Measure one checkout of shout-sh, so two checkouts (two cfonts versions,
# say) can be compared:
#
#   scripts/bench-compare.sh <checkout> <out dir>
#
# It builds the Worker and the playground wasm in <checkout>, then
# records, in <out dir>/results.json and <out dir>/results.md:
#
# - native: the criterion benches in shout-core/benches/render.rs, run by
#   `just bench` in <checkout>. Median and spread over criterion's samples.
# - sizes: the Worker wasm and the playground wasm, raw, gzip -9 and
#   brotli -q 11.
# - worker: CPU time per request for the built Worker, run in Node by
#   scripts/bench-worker.mjs. A local stand-in for Workers CPU time, not
#   the same number; see that file.
#
# The scripts this one calls are taken from next to it, not from
# <checkout>, so it runs unchanged on a checkout that does not have them.
# SAMPLES sets the samples per Worker request (default 15).
set -euo pipefail

if [[ $# -ne 2 ]]; then
	echo "usage: $0 <checkout> <out dir>" >&2
	exit 2
fi
here="$(cd "$(dirname "$0")" && pwd)"
checkout="$(cd "$1" && pwd)"
mkdir -p "$2"
out="$(cd "$2" && pwd)"
samples="${SAMPLES:-15}"

for tool in just cargo rustc node jq gzip brotli wasm-pack worker-build; do
	command -v "$tool" >/dev/null || {
		echo "missing: $tool" >&2
		exit 1
	}
done

log="$out/build.log"
: >"$log"
step() { echo "== $*" | tee -a "$log" >&2; }

step "build Worker and playground wasm"
(cd "$checkout" && just worker-build && just wasm-build) >>"$log" 2>&1

step "sizes"
size_of() {
	jq -n --arg file "${1#"$checkout"/}" \
		--argjson raw "$(wc -c <"$1")" \
		--argjson gzip "$(gzip -9 -c "$1" | wc -c)" \
		--argjson brotli "$(brotli -q 11 -c "$1" | wc -c)" \
		'{file: $file, raw: $raw, gzip: $gzip, brotli: $brotli}'
}
jq -n --argjson worker "$(size_of "$checkout/shout-worker/build/index_bg.wasm")" \
	--argjson playground "$(size_of "$checkout/shout-wasm/pkg/shout_wasm_bg.wasm")" \
	'{worker: $worker, playground: $playground}' >"$out/sizes.json"

step "worker CPU per request ($samples samples each)"
node "$here/bench-worker.mjs" "$checkout/shout-worker/build" "$samples" >"$out/worker.json"

step "native benches (criterion)"
# A baseline name of its own, so only this run's results are read back,
# and any baselines already in <checkout>/target/criterion are left alone.
baseline="bench-compare-$$"
(cd "$checkout" && just bench -- --noplot --save-baseline "$baseline") >"$out/criterion.log" 2>&1
target="$(cd "$checkout" && cargo metadata --format-version 1 --no-deps | jq -r .target_directory)"
native="[]"
shopt -s nullglob
for dir in "$target"/criterion/*/*/"$baseline"; do
	native="$(jq --slurpfile b "$dir/benchmark.json" --slurpfile e "$dir/estimates.json" \
		'. + [{name: $b[0].full_id,
			median_ns: $e[0].median.point_estimate,
			ci95_ns: [$e[0].median.confidence_interval.lower_bound, $e[0].median.confidence_interval.upper_bound],
			mad_ns: $e[0].median_abs_dev.point_estimate}]' <<<"$native")"
	rm -rf "$dir"
done
if [[ "$native" == "[]" ]]; then
	echo "no criterion results under $target/criterion; see $out/criterion.log" >&2
	exit 1
fi

step "write results"
jq -n \
	--arg date "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
	--arg arch "$(uname -m)" \
	--arg os "$(uname -sr)" \
	--arg cpu "$(sysctl -n machdep.cpu.brand_string 2>/dev/null || grep -m1 'model name' /proc/cpuinfo | cut -d: -f2- | sed 's/^ //')" \
	--arg rustc "$(cd "$checkout" && rustc -V)" \
	--arg commit "$(git -C "$checkout" rev-parse --short HEAD)" \
	--arg branch "$(git -C "$checkout" rev-parse --abbrev-ref HEAD)" \
	--arg dirty "$(git -C "$checkout" status --porcelain --untracked-files=no | wc -l | tr -d ' ')" \
	--arg cfonts "$(awk '/^name = "cfonts"/ { getline; print $3 }' "$checkout/Cargo.lock" | tr -d '"' | paste -sd, -)" \
	--argjson native "$native" \
	--slurpfile sizes "$out/sizes.json" \
	--slurpfile worker "$out/worker.json" \
	'{machine: {date: $date, arch: $arch, os: $os, cpu: $cpu, rustc: $rustc},
	  checkout: {commit: $commit, branch: $branch, modified_files: ($dirty | tonumber), cfonts: $cfonts},
	  native: ($native | sort_by(.name)), sizes: $sizes[0], worker: $worker[0]}' >"$out/results.json"
rm "$out/sizes.json" "$out/worker.json"

# The same numbers as a markdown table, for reading.
jq -r '
	def ms: . * 1000 | round / 1000 | tostring;
	def us: . / 1000 | . * 100 | round / 100 | tostring;
	def kb: . / 1024 | . * 10 | round / 10 | tostring;
	"# shout-sh bench: cfonts \(.checkout.cfonts) at \(.checkout.commit) (\(.checkout.branch))",
	"",
	"\(.machine.cpu), \(.machine.arch), \(.machine.os); \(.machine.rustc); Node \(.worker.node) (V8 \(.worker.v8)); \(.machine.date)",
	"",
	"## Native render (criterion, µs)",
	"",
	"| bench | median | 95% CI | MAD |",
	"|---|---:|---:|---:|",
	(.native[] | "| \(.name) | \(.median_ns | us) | \(.ci95_ns[0] | us)–\(.ci95_ns[1] | us) | \(.mad_ns | us) |"),
	"",
	"## Wasm size (KiB)",
	"",
	"| file | raw | gzip -9 | brotli -q 11 |",
	"|---|---:|---:|---:|",
	(.sizes[] | "| \(.file) | \(.raw | kb) | \(.gzip | kb) | \(.brotli | kb) |"),
	"",
	"## Worker CPU per request (Node V8, ms, \(.worker.samples) samples)",
	"",
	"| request | median | min | max | batch | bytes |",
	"|---|---:|---:|---:|---:|---:|",
	(.worker.startup.decode_validate_ms.cpu | "| wasm decode and validate (startup, lazy compile) | \(.median | ms) | \(.min | ms) | \(.max | ms) | 1 | |"),
	(.worker.startup.full_compile_ms.cpu | "| wasm full compile (startup, all threads) | \(.median | ms) | \(.min | ms) | \(.max | ms) | 1 | |"),
	(.worker.requests[] | "| \(.name) | \(.cpu_ms.median | ms) | \(.cpu_ms.min | ms) | \(.cpu_ms.max | ms) | \(.batch) | \(.bytes) |")
' "$out/results.json" >"$out/results.md"

step "done: $out/results.md"
