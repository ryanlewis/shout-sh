// CurlSnippet — live curl string + copy-to-clipboard.

import { buildCurl } from '../urls.js';
import type { UrlState } from '../urls.js';

export interface CurlSnippetOpts {
	display: HTMLElement;
	button: HTMLButtonElement;
	log: HTMLElement;
	onCopy?: (state: UrlState) => void;
}

export function mountCurlSnippet(opts: CurlSnippetOpts): (state: UrlState) => void {
	let current = '';
	let currentState: UrlState | null = null;

	opts.button.addEventListener('click', () => {
		void navigator.clipboard
			?.writeText(current)
			.then(() => {
				appendLog(opts.log, `[OK] copied: ${current}`);
				if (currentState) opts.onCopy?.(currentState);
			})
			.catch((e: unknown) => appendLog(opts.log, `[ERR] copy failed: ${String(e)}`));
	});

	return (state: UrlState) => {
		current = buildCurl(state);
		currentState = state;
		opts.display.textContent = current;
	};
}

function appendLog(root: HTMLElement, line: string): void {
	const el = document.createElement('div');
	el.className = 'log__line';
	el.textContent = line;
	root.prepend(el);
	// Keep the log bounded — we don't need an audit trail.
	while (root.children.length > 5) root.lastElementChild?.remove();
}
