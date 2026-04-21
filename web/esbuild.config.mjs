// esbuild driver for the shout.sh playground. Bundles TS + CSS, copies the
// wasm glue and the raw .wasm, and stamps out a flat dist/ that the Rust
// server embeds with include_bytes!.

import { context, build } from 'esbuild';
import { cp, mkdir, rm, readFile, writeFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const root = dirname(fileURLToPath(import.meta.url));
const dist = join(root, 'dist');
const wasmPkg = join(root, 'src', 'wasm-pkg');
const watch = process.argv.includes('--watch');

async function clean() {
	await rm(dist, { recursive: true, force: true });
	await mkdir(dist, { recursive: true });
}

async function copyStatic() {
	await cp(join(root, 'index.html'), join(dist, 'index.html'));
	if (!existsSync(wasmPkg)) {
		throw new Error(`missing ${wasmPkg} — run \`just wasm-build\` from the repo root first`);
	}
	await cp(join(wasmPkg, 'shout_wasm.js'), join(dist, 'shout_wasm.js'));
	await cp(join(wasmPkg, 'shout_wasm_bg.wasm'), join(dist, 'shout_wasm_bg.wasm'));
}

const esbuildOpts = {
	entryPoints: [join(root, 'src', 'main.ts'), join(root, 'src', 'styles.css')],
	entryNames: '[name]',
	bundle: true,
	format: 'esm',
	target: 'es2022',
	outdir: dist,
	sourcemap: watch ? 'inline' : false,
	minify: !watch,
	logLevel: 'info',
	// wasm glue uses import.meta.url to resolve the .wasm alongside it — keep
	// its imports external so the dynamic URL resolution works at runtime.
	external: ['*.wasm'],
	loader: { '.css': 'css' },
};

// The generated glue references styles.css output as main.css via bundle.
// Rename the CSS output file to match what index.html expects.
await clean();
await copyStatic();

if (watch) {
	const ctx = await context(esbuildOpts);
	await ctx.watch();
	console.log('esbuild watching…');
} else {
	await build(esbuildOpts);
	// esbuild produces styles.css; rename to main.css for index.html's link tag.
	const stylesPath = join(dist, 'styles.css');
	const mainCssPath = join(dist, 'main.css');
	if (existsSync(stylesPath)) {
		const body = await readFile(stylesPath);
		await writeFile(mainCssPath, body);
		await rm(stylesPath);
	}
}
