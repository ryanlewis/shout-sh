// esbuild driver for the shout.sh playground. Bundles TS + CSS, hashes the
// runtime-fetched wasm, stamps the hashed filenames into the HTML, and lays
// out a flat dist/ that the Rust server embeds via build.rs.

import { context, build } from 'esbuild';
import { cp, mkdir, rm, readFile, writeFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { dirname, join, basename } from 'node:path';
import { Resvg } from '@resvg/resvg-js';

const root = dirname(fileURLToPath(import.meta.url));
const dist = join(root, 'dist');
const distApp = join(dist, '_app');
const assets = join(root, 'assets');
const wasmPkg = join(root, 'src', 'wasm-pkg');
const watch = process.argv.includes('--watch');

const HTML_FILES = ['index.html', 'about.html', 'privacy.html'];
const APP_URL_PREFIX = '/_app';

async function clean() {
	await rm(dist, { recursive: true, force: true });
	await mkdir(distApp, { recursive: true });
}

function shortHash(bytes) {
	return createHash('sha256').update(bytes).digest('hex').slice(0, 8);
}

async function copyTopLevelHtml() {
	for (const name of HTML_FILES) {
		await cp(join(root, name), join(dist, name));
	}
}

async function copyTopLevelStatic() {
	// favicon.svg ships as-is. The OG card is generated from the raw banner
	// captured from `curl shout.sh/shout.sh` so the preview shows what the
	// tool actually produces. PNG because Slack/Twitter/Facebook don't
	// reliably honor SVG og:images.
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

async function emitHashedWasm() {
	if (!existsSync(wasmPkg)) {
		throw new Error(`missing ${wasmPkg} — run \`just wasm-build\` from the repo root first`);
	}
	const bytes = await readFile(join(wasmPkg, 'shout_wasm_bg.wasm'));
	const name = `shout_wasm_bg-${shortHash(bytes)}.wasm`;
	await writeFile(join(distApp, name), bytes);
	return name;
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

function makeEsbuildOpts(wasmUrl) {
	return {
		entryPoints: [join(root, 'src', 'main.ts'), join(root, 'src', 'styles.css')],
		entryNames: '[name]-[hash]',
		bundle: true,
		format: 'esm',
		target: 'es2022',
		outdir: distApp,
		sourcemap: watch ? 'inline' : false,
		minify: !watch,
		metafile: true,
		logLevel: 'info',
		// wasm glue uses import.meta.url to resolve the .wasm alongside it — keep
		// its imports external so the dynamic URL resolution works at runtime.
		external: ['*.wasm'],
		loader: { '.css': 'css' },
		define: { __WASM_URL__: JSON.stringify(wasmUrl) },
	};
}

function emittedNames(metafile) {
	const out = {};
	for (const [path, info] of Object.entries(metafile.outputs)) {
		if (!info.entryPoint) continue;
		const entry = basename(info.entryPoint);
		out[entry] = basename(path);
	}
	return out;
}

async function stampHtml(replacements) {
	for (const name of HTML_FILES) {
		const path = join(dist, name);
		let html = await readFile(path, 'utf8');
		for (const [from, to] of Object.entries(replacements)) {
			html = html.replaceAll(from, to);
		}
		await writeFile(path, html);
	}
}

async function applyBuildResult(result, wasmName) {
	const named = emittedNames(result.metafile);
	const jsName = named['main.ts'];
	const cssName = named['styles.css'];
	if (!jsName || !cssName) {
		throw new Error(`esbuild metafile missing entries: ${JSON.stringify(named)}`);
	}
	await stampHtml({
		'/_app/main.js': `${APP_URL_PREFIX}/${jsName}`,
		'/_app/main.css': `${APP_URL_PREFIX}/${cssName}`,
	});
	// Manifest of canonical → hashed names. shout-server/build.rs reads
	// this so the Rust side never has to pattern-match output filenames.
	const manifest = `main_js=${jsName}\nmain_css=${cssName}\nwasm_bg=${wasmName}\n`;
	await writeFile(join(distApp, 'manifest.txt'), manifest);
}

await clean();
await copyTopLevelHtml();
await copyTopLevelStatic();
const wasmName = await emitHashedWasm();
const wasmUrl = `${APP_URL_PREFIX}/${wasmName}`;

if (watch) {
	const stampPlugin = {
		name: 'stamp-hashed-html',
		setup(build) {
			build.onEnd(async (result) => {
				if (!result.metafile) return;
				// Reset HTML to source so the unhashed placeholder is always there to replace.
				await copyTopLevelHtml();
				await applyBuildResult(result, wasmName);
			});
		},
	};
	const ctx = await context({ ...makeEsbuildOpts(wasmUrl), plugins: [stampPlugin] });
	await ctx.watch();
	console.log('esbuild watching…');
} else {
	const result = await build(makeEsbuildOpts(wasmUrl));
	await applyBuildResult(result, wasmName);
}
