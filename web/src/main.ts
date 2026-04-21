// Bootstrap the playground: load wasm, mount UI, drive Preview from state.

import { loadWasm } from './wasm.js';
import type { PlaygroundCfg } from './wasm.js';
import { buildCurl, type UrlState } from './urls.js';
import { Preview } from './ui/Preview.js';
import { mountFontPicker, type FontName } from './ui/FontPicker.js';
import { mountPresetPicker, type PresetName } from './ui/PresetPicker.js';
import { mountControls, type Mode } from './ui/Controls.js';
import { mountCurlSnippet } from './ui/CurlSnippet.js';
import { mountShouter } from './ui/Shouter.js';
import { mountBlockCaret } from './ui/BlockCaret.js';

const WASM_URL = '/_app/shout_wasm_bg.wasm';

async function main(): Promise<void> {
	const frame = must<HTMLElement>('#frame');
	const textIn = must<HTMLInputElement>('#text-in');
	const fontRoot = must<HTMLElement>('#font-picker');
	const presetRoot = must<HTMLElement>('#preset-picker');
	const modeRoot = must<HTMLElement>('#mode-picker');
	const onceCb = must<HTMLInputElement>('#once-cb');
	const fpsIn = must<HTMLInputElement>('#fps-in');
	const fpsVal = must<HTMLElement>('#fps-val');
	const curlOut = must<HTMLElement>('#curl-cmd');
	const termCmd = must<HTMLElement>('#term-cmd');
	const copyBtn = must<HTMLButtonElement>('#copy-btn');
	const log = must<HTMLElement>('#log');

	mountBlockCaret(textIn);

	if (!matchMedia('(pointer: coarse)').matches) {
		textIn.focus();
		textIn.setSelectionRange(textIn.value.length, textIn.value.length);
	}

	let wasm;
	try {
		wasm = await loadWasm(WASM_URL);
	} catch (e) {
		frame.classList.remove('loading');
		frame.textContent =
			`playground unavailable (${String(e)})\n` + `try: curl shout.sh/rainbow/HELLO`;
		return;
	}

	frame.classList.remove('loading');
	frame.parentElement?.removeAttribute('aria-busy');
	const preview = new Preview({ wasm, target: frame });

	const masthead = document.querySelector<HTMLElement>('#masthead-art');
	if (masthead) {
		const fonts = ['block', 'tiny', 'chrome'] as const;
		const font = fonts[Math.floor(Math.random() * fonts.length)]!;
		masthead.innerHTML = wasm.renderOnce({
			text: 'shout.sh',
			font,
			mode: 'solid',
			color: '',
			preset: '',
		});
	}

	const renderCurl = mountCurlSnippet({ display: curlOut, button: copyBtn, log });

	const push = (state: UrlState): void => {
		const cfg: PlaygroundCfg = {
			text: state.text || 'HELLO',
			font: state.font,
			mode: state.mode,
			color: state.color,
			preset: state.preset,
		};
		preview.update(cfg, { fps: state.fps, once: state.once });
		renderCurl(state);
		// Mirror the curl command inside the fake prompt — sells the terminal
		// metaphor, and matches the copy-snippet below verbatim.
		termCmd.textContent = buildCurl(state);
	};

	const shouter = mountShouter({
		input: textIn,
		initial: {
			font: 'block',
			mode: 'solid',
			color: '',
			preset: '',
			once: false,
			fps: 10,
		},
		onChange: push,
	});

	mountFontPicker(fontRoot, 'block', (font: FontName) => shouter.setFont(font));
	const presetPicker = mountPresetPicker(presetRoot, '', (preset: PresetName) =>
		shouter.setPreset(preset),
	);
	mountControls({
		modeRoot,
		onceCheckbox: onceCb,
		fpsInput: fpsIn,
		fpsValue: fpsVal,
		onModeChange: (mode: Mode) => {
			shouter.setMode(mode);
			// Rainbow/fire override preset at render time; grey out the picker
			// so it's visually clear the palette isn't doing anything.
			presetPicker.setDisabled(mode !== 'solid');
		},
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
