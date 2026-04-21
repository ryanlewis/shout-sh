// Stand-in for the generated wasm-pkg module. Tests should use the Wasm
// interface directly with a hand-rolled mock — if anything actually
// reaches this module in a test, we throw so the bad wiring surfaces.

export default async function init(): Promise<void> {
	throw new Error('wasm stub init() called — tests should inject Wasm directly');
}

export function render_once_html(): string {
	throw new Error('wasm stub render_once_html called');
}

export function render_frame_html(): string {
	throw new Error('wasm stub render_frame_html called');
}
