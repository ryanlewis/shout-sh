// Shouter — debounces the text input and publishes UrlState updates.

import type { UrlState } from '../urls.js';
import type { FontName } from './FontPicker.js';
import type { Mode } from './Controls.js';
import type { AdvancedState } from './Advanced.js';

export interface ShouterOpts {
	input: HTMLInputElement;
	initial: Omit<UrlState, 'text'>;
	onChange: (state: UrlState) => void;
	debounceMs?: number;
}

export interface ShouterHandle {
	setFont(font: FontName): void;
	setMode(mode: Mode): void;
	setPreset(preset: string): void;
	setOnce(once: boolean): void;
	setFps(fps: number): void;
	setAdvanced(advanced: AdvancedState): void;
	state(): UrlState;
}

export function mountShouter(opts: ShouterOpts): ShouterHandle {
	const debounce = opts.debounceMs ?? 150;
	let state: UrlState = { ...opts.initial, text: opts.input.value };
	let timer: ReturnType<typeof setTimeout> | null = null;

	const flush = (): void => {
		opts.onChange(state);
	};

	opts.input.addEventListener('input', () => {
		state = { ...state, text: opts.input.value };
		if (timer) clearTimeout(timer);
		timer = setTimeout(flush, debounce);
	});

	// Fire once on mount so everything downstream paints.
	queueMicrotask(flush);

	return {
		setFont(font) {
			state = { ...state, font };
			flush();
		},
		setMode(mode) {
			state = { ...state, mode };
			flush();
		},
		setPreset(preset) {
			state = { ...state, preset };
			flush();
		},
		setOnce(once) {
			state = { ...state, once };
			flush();
		},
		setFps(fps) {
			state = { ...state, fps };
			flush();
		},
		setAdvanced(advanced) {
			state = { ...state, ...advanced };
			flush();
		},
		state() {
			return state;
		},
	};
}
