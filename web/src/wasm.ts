// Thin wrapper around shout-wasm's generated JS glue. The init step
// fetches /_app/shout_wasm_bg.wasm at runtime so it's served with the
// correct Content-Type header by the Rust server.

import init, { render_frame_html, render_once_html } from '@wasm/shout_wasm.js';

export interface Wasm {
	renderOnce(cfg: PlaygroundCfg): string;
	renderFrame(cfg: PlaygroundCfg, frame: number): string;
}

export interface PlaygroundCfg {
	text: string;
	font: string;
	mode: 'solid' | 'rainbow' | 'fire';
	color: string;
}

export async function loadWasm(wasmUrl: string): Promise<Wasm> {
	await init({ module_or_path: wasmUrl });
	return {
		renderOnce: (cfg) => render_once_html(JSON.stringify(cfg)),
		renderFrame: (cfg, frame) => render_frame_html(JSON.stringify(cfg), frame),
	};
}
