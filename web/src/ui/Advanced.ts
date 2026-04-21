// Advanced options panel: letter-spacing, padding, max-length, background.
// The panel itself is a <details> element in the HTML; this module just wires
// its inputs to callbacks.

export interface AdvancedState {
	letterSpacing: number;
	maxLength: number;
	padding: number;
	background: string;
}

export interface AdvancedOpts {
	letterSpacingInput: HTMLInputElement;
	letterSpacingValue: HTMLElement;
	paddingInput: HTMLInputElement;
	paddingValue: HTMLElement;
	maxLengthInput: HTMLInputElement;
	backgroundSelect: HTMLSelectElement;
	onChange: (state: AdvancedState) => void;
}

export function mountAdvanced(opts: AdvancedOpts): AdvancedState {
	const state: AdvancedState = {
		letterSpacing: Number(opts.letterSpacingInput.value),
		maxLength: Number(opts.maxLengthInput.value),
		padding: Number(opts.paddingInput.value),
		background: opts.backgroundSelect.value,
	};

	opts.letterSpacingInput.addEventListener('input', () => {
		const n = Number(opts.letterSpacingInput.value);
		opts.letterSpacingValue.textContent = String(n);
		state.letterSpacing = n;
		opts.onChange({ ...state });
	});

	opts.paddingInput.addEventListener('input', () => {
		const n = Number(opts.paddingInput.value);
		opts.paddingValue.textContent = String(n);
		state.padding = n;
		opts.onChange({ ...state });
	});

	opts.maxLengthInput.addEventListener('input', () => {
		const raw = Number(opts.maxLengthInput.value);
		const n = Number.isFinite(raw) ? Math.max(0, Math.min(200, Math.trunc(raw))) : 0;
		state.maxLength = n;
		opts.onChange({ ...state });
	});

	opts.backgroundSelect.addEventListener('change', () => {
		state.background = opts.backgroundSelect.value;
		opts.onChange({ ...state });
	});

	return { ...state };
}

/// Named ANSI colors → CSS colors, for the playground preview background.
/// Server rendering uses cfonts' own BG SGR; this map only mirrors it in-browser.
export const BG_CSS: Record<string, string> = {
	'': 'transparent',
	black: '#000000',
	red: '#aa0000',
	green: '#00aa00',
	yellow: '#aa5500',
	blue: '#0000aa',
	magenta: '#aa00aa',
	cyan: '#00aaaa',
	white: '#aaaaaa',
	gray: '#555555',
	redbright: '#ff5555',
	greenbright: '#55ff55',
	yellowbright: '#ffff55',
	bluebright: '#5555ff',
	magentabright: '#ff55ff',
	cyanbright: '#55ffff',
	whitebright: '#ffffff',
};
