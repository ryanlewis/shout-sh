// Wires up the mode radio group, once checkbox, and fps slider to callbacks.

import type { PlaygroundCfg } from '../wasm.js';

export type Mode = PlaygroundCfg['mode'];

export interface ControlsOpts {
	modeRoot: HTMLElement;
	onceCheckbox: HTMLInputElement;
	fpsInput: HTMLInputElement;
	fpsValue: HTMLElement;
	onModeChange: (mode: Mode) => void;
	onOnceChange: (once: boolean) => void;
	onFpsChange: (fps: number) => void;
}

export function mountControls(opts: ControlsOpts): void {
	const buttons = Array.from(
		opts.modeRoot.querySelectorAll<HTMLButtonElement>('button[data-mode]'),
	);
	for (const btn of buttons) {
		btn.addEventListener('click', () => {
			const mode = btn.getAttribute('data-mode') as Mode | null;
			if (!mode) return;
			for (const b of buttons) {
				b.setAttribute('aria-checked', String(b === btn));
			}
			opts.onModeChange(mode);
		});
	}

	opts.onceCheckbox.addEventListener('change', () => {
		const checked = opts.onceCheckbox.checked;
		const label = opts.onceCheckbox.closest('.opt')?.querySelector('.check__box');
		if (label) label.textContent = checked ? '[x]' : '[ ]';
		opts.onOnceChange(checked);
	});

	opts.fpsInput.addEventListener('input', () => {
		const n = Number(opts.fpsInput.value);
		opts.fpsValue.textContent = String(n);
		opts.onFpsChange(n);
	});
}
