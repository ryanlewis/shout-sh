// esbuild driver for the shout.sh playground. Bundles TS + CSS, copies the
// wasm glue and the raw .wasm, and stamps out a flat dist/ that the Rust
// server embeds with include_bytes!.

import { context, build } from 'esbuild';
import { cp, mkdir, rm, readFile, writeFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { Resvg } from '@resvg/resvg-js';

const root = dirname(fileURLToPath(import.meta.url));
const dist = join(root, 'dist');
const assets = join(root, 'assets');
const wasmPkg = join(root, 'src', 'wasm-pkg');
const watch = process.argv.includes('--watch');

async function clean() {
	await rm(dist, { recursive: true, force: true });
	await mkdir(dist, { recursive: true });
}

async function copyStatic() {
	await cp(join(root, 'index.html'), join(dist, 'index.html'));
	await cp(join(root, 'privacy.html'), join(dist, 'privacy.html'));
	await cp(join(root, 'about.html'), join(dist, 'about.html'));
	if (!existsSync(wasmPkg)) {
		throw new Error(`missing ${wasmPkg} — run \`just wasm-build\` from the repo root first`);
	}
	await cp(join(wasmPkg, 'shout_wasm.js'), join(dist, 'shout_wasm.js'));
	await cp(join(wasmPkg, 'shout_wasm_bg.wasm'), join(dist, 'shout_wasm_bg.wasm'));

	// favicon.svg ships as-is. The OG card is generated from the raw banner
	// captured from `curl shout.sh/shout.sh` (committed at og-banner.txt) so
	// the preview shows what the tool actually produces, not marketing copy.
	// Rasterize to PNG because Slack / Twitter / Facebook don't reliably
	// honor SVG og:images.
	await cp(join(assets, 'favicon.svg'), join(dist, 'favicon.svg'));
	const ogSvg = buildOgSvg(await readFile(join(assets, 'og-banner.txt'), 'utf8'));
	const fontsDir = join(assets, 'fonts');
	const png = new Resvg(ogSvg, {
		fitTo: { mode: 'width', value: 1200 },
		font: {
			loadSystemFonts: false,
			fontFiles: [
				join(fontsDir, 'JetBrainsMono-Regular.ttf'),
				join(fontsDir, 'JetBrainsMono-Bold.ttf'),
			],
			defaultFontFamily: 'JetBrains Mono',
		},
	})
		.render()
		.asPng();
	await writeFile(join(dist, 'og.png'), png);
}

function buildOgSvg(bannerRaw) {
	const lines = bannerRaw
		.replace(/\r\n/g, '\n')
		.split('\n')
		// trim only leading/trailing empty lines; preserve intra-banner spacing
		.filter((_, i, a) => {
			const firstNonEmpty = a.findIndex((l) => l.trim().length > 0);
			const lastNonEmpty = a.length - 1 - [...a].reverse().findIndex((l) => l.trim().length > 0);
			return i >= firstNonEmpty && i <= lastNonEmpty;
		});

	const xmlEscape = (s) =>
		s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');

	const bannerFontSize = 24;
	const bannerLineHeight = Math.round(bannerFontSize * 1.15);
	const bannerStartY = 230;

	const tspans = lines
		.map((line, i) => {
			const dy = i === 0 ? 0 : bannerLineHeight;
			return `<tspan x="60" dy="${dy}">${xmlEscape(line)}</tspan>`;
		})
		.join('');

	return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1200 630" width="1200" height="630">
	<rect width="1200" height="630" fill="#000"/>
	<rect x="1" y="1" width="1198" height="628" fill="none" stroke="#2e2e2e" stroke-width="2"/>
	<g font-family="JetBrains Mono" font-weight="500">
		<text x="60" y="110" font-size="28" fill="#8a8a8a">guest@shout:~$ curl shout.sh/shout.sh</text>
		<text xml:space="preserve" y="${bannerStartY}" font-size="${bannerFontSize}" fill="#f5f5f5" font-weight="700">${tspans}</text>
		<text x="60" y="580" font-size="32" fill="#d0d0d0">render sexy text in your terminal.</text>
	</g>
</svg>`;
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

// esbuild emits styles.css; the server and index.html both expect main.css.
// Rename on every build (including watch rebuilds) so the dev loop works.
async function renameStylesToMain() {
	const stylesPath = join(dist, 'styles.css');
	if (!existsSync(stylesPath)) return;
	const body = await readFile(stylesPath);
	await writeFile(join(dist, 'main.css'), body);
	await rm(stylesPath);
}

if (watch) {
	const renamePlugin = {
		name: 'rename-styles-to-main',
		setup(build) {
			build.onEnd(() => renameStylesToMain());
		},
	};
	const ctx = await context({ ...esbuildOpts, plugins: [renamePlugin] });
	await ctx.watch();
	console.log('esbuild watching…');
} else {
	await build(esbuildOpts);
	await renameStylesToMain();
}
