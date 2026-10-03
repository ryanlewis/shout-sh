// CPU time per request for the built Worker (shout-worker/build), run in
// Node's V8 instead of workerd. Called by scripts/bench-compare.sh.
//
//   node scripts/bench-worker.mjs <build dir> [samples]
//
// Prints JSON to stdout. Each request runs `batch` times per sample, and
// a sample is process CPU time (user + system) over the batch divided by
// the batch size. This is a local stand-in for Workers CPU time, not the
// same number: Node's V8 is older than workerd's, there is no workerd
// I/O layer, and process CPU includes V8's GC and compiler threads.
//
// env is empty and no cf-connecting-ip header is sent, so the rate
// limits, stream slots and Analytics Engine writes are all skipped, as
// the glue fails open without them. What is left is routing, rendering
// and the wasm-bindgen glue.

import { execFileSync } from "node:child_process";
import { register } from "node:module";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

const [buildDir, samplesArg] = process.argv.slice(2);
if (!buildDir) {
	console.error("usage: bench-worker.mjs <build dir> [samples]");
	process.exit(2);
}
const SAMPLES = Number(samplesArg ?? 15);
if (!Number.isInteger(SAMPLES) || SAMPLES < 1) {
	console.error(`samples must be a whole number of at least 1, not ${samplesArg}`);
	process.exit(2);
}

// worker-build's index.js imports `cloudflare:workers` and the wasm file
// as a module. Node has neither, so serve both from a loader hook.
const hooks = `
export async function resolve(spec, ctx, next) {
	if (spec === "cloudflare:workers") return { url: "bench:workers", shortCircuit: true };
	return next(spec, ctx);
}
export async function load(url, ctx, next) {
	if (url === "bench:workers") {
		return {
			format: "module",
			shortCircuit: true,
			source: "export class WorkerEntrypoint { constructor(ctx, env) { this.ctx = ctx; this.env = env; } }",
		};
	}
	if (url.endsWith(".wasm")) {
		return {
			format: "module",
			shortCircuit: true,
			source: "import { readFileSync } from 'node:fs';" +
				"export default new WebAssembly.Module(readFileSync(new URL(" + JSON.stringify(url) + ")));",
		};
	}
	return next(url, ctx);
}`;
register(`data:text/javascript,${encodeURIComponent(hooks)}`);
// The glue listens for uncaught errors on the global scope.
globalThis.addEventListener ??= () => {};

const wasmPath = resolve(buildDir, "index_bg.wasm");
const cpuMs = (start) => {
	const d = process.cpuUsage(start);
	return (d.user + d.system) / 1000;
};


const CURL = { "user-agent": "curl/8.7.1", accept: "*/*" };
const BROWSER = {
	"user-agent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)",
	accept: "text/html,application/xhtml+xml,*/*;q=0.8",
};
const LONG = "the+quick+brown+fox+jumps+over+the+lazy+dog";
const HEAVY = `/rainbow+3d/${"W".repeat(200)}?ls=10&pad=10&fps=30&timeout=300`;

// [name, path, headers, frames]. frames 0 reads the whole body; n > 0
// reads n chunks of a stream and cancels it.
const CASES = [
	["health", "/health", CURL, 0],
	["simple/short/solid", "/simple/shout", CURL, 0],
	["block/short/solid", "/block/shout", CURL, 0],
	["block/long/solid", `/block/${LONG}`, CURL, 0],
	["chrome/short/solid", "/chrome/shout", CURL, 0],
	["tiny/long/solid", `/tiny/${LONG}`, CURL, 0],
	["block/short/red", "/block+red/shout", CURL, 0],
	["block/short/sunset", "/block+sunset/shout", CURL, 0],
	["simple/long/sunset", `/simple+sunset/${LONG}`, CURL, 0],
	["block/short/rainbow-once", "/block+rainbow+once/shout", CURL, 0],
	["simple/long/fire-once", `/simple+fire+once/${LONG}`, CURL, 0],
	["block/short/json", "/block/shout?format=json", CURL, 0],
	["browser/block/short/rainbow", "/block+rainbow/shout", BROWSER, 0],
	["stream/block/rainbow/frame0", "/block+rainbow/shout.sh", CURL, 1],
	["stream/simple/fire/frame0", `/simple+fire/${LONG}`, CURL, 1],
	["stream/3d/rainbow/heavy/frame0", HEAVY, CURL, 1],
];

// Run one request and drain `frames` chunks (0: the whole body).
async function run(path, headers, frames) {
	const res = await worker.fetch(new Request(`http://localhost${path}`, { headers }));
	if (res.status !== 200) throw new Error(`${path}: status ${res.status}`);
	const reader = res.body.getReader();
	let bytes = 0;
	for (let n = 0; frames === 0 || n < frames; n++) {
		const { done, value } = await reader.read();
		if (done) break;
		bytes += value.byteLength;
	}
	if (frames > 0) await reader.cancel();
	return bytes;
}

const stats = (xs) => {
	const s = [...xs].sort((a, b) => a - b);
	const mid = s.length >> 1;
	const median = s.length % 2 ? s[mid] : (s[mid - 1] + s[mid]) / 2;
	return { median, min: s[0], max: s.at(-1), n: s.length };
};
const round = (o) => Object.fromEntries(Object.entries(o).map(([k, v]) => [k, k === "n" ? v : Number(v.toFixed(4))]));

// Startup: compile the wasm, which index.js does at import. A Worker pays
// it once per isolate. Each sample is a fresh Node process, so V8's module
// cache cannot answer it. By default V8 compiles wasm lazily: compile()
// only decodes and validates, and functions compile on first call. With
// --no-wasm-lazy-compilation it compiles every function up front, on
// several threads, so CPU time is well above wall time.
const COMPILE = `import { readFileSync } from "node:fs";
const bytes = readFileSync(process.argv[1]);
const t = process.cpuUsage(), w = performance.now();
await WebAssembly.compile(bytes);
const d = process.cpuUsage(t);
console.log(JSON.stringify({ cpu: (d.user + d.system) / 1000, wall: performance.now() - w }));`;
const compile = (flags) => {
	const runs = [];
	for (let i = 0; i < SAMPLES; i++) {
		const out = execFileSync(process.execPath, [...flags, "--input-type=module", "-e", COMPILE, wasmPath]);
		runs.push(JSON.parse(out));
	}
	return { cpu: round(stats(runs.map((r) => r.cpu))), wall: round(stats(runs.map((r) => r.wall))) };
};
const startup = {
	decode_validate_ms: compile([]),
	full_compile_ms: compile(["--no-wasm-lazy-compilation"]),
};

const mod = await import(pathToFileURL(resolve(buildDir, "index.js")).href);
const ctx = { waitUntil() {}, passThroughOnException() {} };
const worker = new mod.default(ctx, {});

const requests = [];
for (const [name, path, headers, frames] of CASES) {
	const bytes = await run(path, headers, frames);
	// Warm up the JIT for about 200ms of CPU, then size the batch so one
	// sample takes about 50ms, which keeps timer resolution out of it.
	const warm = process.cpuUsage();
	let runs = 0;
	while (cpuMs(warm) < 200) {
		await run(path, headers, frames);
		runs++;
	}
	const batch = Math.max(1, Math.round((50 * runs) / cpuMs(warm)));
	const samples = [];
	for (let s = 0; s < SAMPLES; s++) {
		const t0 = process.cpuUsage();
		for (let i = 0; i < batch; i++) await run(path, headers, frames);
		samples.push(cpuMs(t0) / batch);
	}
	requests.push({ name, path, bytes, batch, cpu_ms: round(stats(samples)) });
}

console.log(
	JSON.stringify(
		{
			node: process.version,
			v8: process.versions.v8,
			samples: SAMPLES,
			startup,
			requests,
		},
		null,
		2,
	),
);
