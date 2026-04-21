// Bootstrap the playground: load wasm, mount UI, drive Preview from state.

import { loadWasm } from './wasm.js';
import type { PlaygroundCfg } from './wasm.js';
import type { UrlState } from './urls.js';
import { Preview } from './ui/Preview.js';
import { mountFontPicker, type FontName } from './ui/FontPicker.js';
import { mountControls, type Mode } from './ui/Controls.js';
import { mountCurlSnippet } from './ui/CurlSnippet.js';
import { mountShouter } from './ui/Shouter.js';

const WASM_URL = '/_app/shout_wasm_bg.wasm';

async function main(): Promise<void> {
	const frame = must<HTMLElement>('#frame');
	const textIn = must<HTMLInputElement>('#text-in');
	const fontRoot = must<HTMLElement>('#font-picker');
	const modeRoot = must<HTMLElement>('#mode-picker');
	const onceCb = must<HTMLInputElement>('#once-cb');
	const fpsIn = must<HTMLInputElement>('#fps-in');
	const fpsVal = must<HTMLElement>('#fps-val');
	const curlOut = must<HTMLElement>('#curl-cmd');
	const copyBtn = must<HTMLButtonElement>('#copy-btn');
	const log = must<HTMLElement>('#log');

	let wasm;
	try {
		wasm = await loadWasm(WASM_URL);
	} catch (e) {
		frame.innerHTML = '';
		const pre = document.createElement('pre');
		pre.className = 'shout-frame';
		pre.textContent =
			`playground unavailable (${String(e)})\n` + `try: curl shout.sh/rainbow/HELLO`;
		frame.appendChild(pre);
		return;
	}

	frame.removeAttribute('aria-busy');
	const preview = new Preview({ wasm, target: frame });

	const renderCurl = mountCurlSnippet({ display: curlOut, button: copyBtn, log });

	const push = (state: UrlState): void => {
		const cfg: PlaygroundCfg = {
			text: state.text || 'HELLO',
			font: state.font,
			mode: state.mode,
			color: state.color,
		};
		preview.update(cfg, { fps: state.fps, once: state.once });
		renderCurl(state);
	};

	const shouter = mountShouter({
		input: textIn,
		initial: {
			font: 'block',
			mode: 'rainbow',
			color: '',
			once: false,
			fps: 10,
		},
		onChange: push,
	});

	mountFontPicker(fontRoot, 'block', (font: FontName) => shouter.setFont(font));
	mountControls({
		modeRoot,
		onceCheckbox: onceCb,
		fpsInput: fpsIn,
		fpsValue: fpsVal,
		onModeChange: (mode: Mode) => shouter.setMode(mode),
		onOnceChange: (once) => shouter.setOnce(once),
		onFpsChange: (fps) => shouter.setFps(fps),
	});
}

function must<T extends HTMLElement>(sel: string): T {
	const el = document.querySelector<T>(sel);
	if (!el) throw new Error(`missing element: ${sel}`);
	return el;
}

void main();
