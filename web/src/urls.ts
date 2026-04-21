// Build the canonical curl URL that mirrors what the server would render
// for the current playground state. This is the ONLY place the public URL
// shape is assembled — keep it dumb and pure so it's easy to test.

import type { PlaygroundCfg } from './wasm.js';

export interface UrlState extends PlaygroundCfg {
	once: boolean;
	fps: number;
}

const DEFAULT_FONT = 'block';
const DEFAULT_FPS = 10;

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
	const query = state.fps !== DEFAULT_FPS ? `?fps=${state.fps}` : '';
	return `/${head}${text}${query}`;
}

export function buildCurl(state: UrlState, host = 'shout.sh'): string {
	return `curl ${host}${buildPath(state)}`;
}
