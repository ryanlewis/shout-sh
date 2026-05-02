// Build the canonical curl URL that mirrors what the server would render
// for the current playground state. This is the ONLY place the public URL
// shape is assembled — keep it dumb and pure so it's easy to test.

import type { PlaygroundCfg } from './wasm.js';

export interface UrlState extends PlaygroundCfg {
	once: boolean;
	fps: number;
	letterSpacing: number;
	maxLength: number;
	padding: number;
	background: string;
}

const DEFAULT_FONT = 'block';
const DEFAULT_FPS = 10;
const DEFAULT_LETTER_SPACING = 1;
const DEFAULT_PADDING = 2;

export function buildPath(state: UrlState): string {
	const directives: string[] = [];
	if (state.font && state.font !== DEFAULT_FONT) directives.push(state.font);
	if (state.mode !== 'solid') directives.push(state.mode);
	// Preset and bare color only make sense when the effect is solid/none.
	// Rainbow/fire override them at render time, so don't pollute the URL.
	if (state.mode === 'solid') {
		if (state.preset) directives.push(state.preset);
		else if (state.color) directives.push(state.color);
	}
	if (state.once) directives.push('once');

	const text = encodeURIComponent(state.text || '').replace(/%20/g, '+');
	const head = directives.length ? `${directives.join('+')}/` : '';

	const params: string[] = [];
	if (state.fps !== DEFAULT_FPS) params.push(`fps=${state.fps}`);
	if (state.letterSpacing !== DEFAULT_LETTER_SPACING)
		params.push(`spacing=${state.letterSpacing}`);
	if (state.padding !== DEFAULT_PADDING) params.push(`padding=${state.padding}`);
	if (state.maxLength > 0) params.push(`maxlength=${state.maxLength}`);
	if (state.background) params.push(`bg=${state.background}`);
	const query = params.length ? `?${params.join('&')}` : '';
	return `/${head}${text}${query}`;
}

const SHELL_UNSAFE = /[&?*[\](){};|<>$`\\"'\s~#!]/;

// encodeURIComponent leaves `'` (and * ( ) ! ~) untouched, so a user-typed
// quote survives into the URL — close-quote / escaped-quote / re-open is
// the POSIX-safe way to embed it inside single quotes.
function shellQuoteIfNeeded(s: string): string {
	return SHELL_UNSAFE.test(s) ? `'${s.replace(/'/g, `'\\''`)}'` : s;
}

export function buildCurl(state: UrlState, host = 'shout.sh'): string {
	return `curl ${shellQuoteIfNeeded(`${host}${buildPath(state)}`)}`;
}
