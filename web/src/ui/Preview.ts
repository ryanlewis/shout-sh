// RAF-driven preview surface. Calls wasm.renderFrame each tick (throttled
// to the configured fps) and swaps the inner HTML of a <pre> target. Every
// byte in the output comes from the wasm emitter, which HTML-escapes every
// cell char — the only DOM sink here is innerHTML of the frame root.

import type { PlaygroundCfg, Wasm } from '../wasm.js';

export interface PreviewDeps {
	wasm: Wasm;
	target: HTMLElement;
	// Injectable for tests.
	raf?: (cb: FrameRequestCallback) => number;
	caf?: (id: number) => void;
}

export class Preview {
	private readonly wasm: Wasm;
	private readonly target: HTMLElement;
	private readonly raf: (cb: FrameRequestCallback) => number;
	private readonly caf: (id: number) => void;

	private cfg: PlaygroundCfg | null = null;
	private fps = 10;
	private once = false;
	private frame = 0;
	private lastTick: number | null = null;
	private rafId = 0;
	private running = false;
	// Bumped on every stop/update so stale RAF callbacks from a prior loop
	// can tell they've been orphaned — cheaper than wrestling with
	// cancelAnimationFrame timing quirks.
	private generation = 0;

	constructor(deps: PreviewDeps) {
		this.wasm = deps.wasm;
		this.target = deps.target;
		this.raf = deps.raf ?? ((cb) => requestAnimationFrame(cb));
		this.caf = deps.caf ?? ((id) => cancelAnimationFrame(id));
	}

	/** Swap the active config + mode. Resets the frame counter. */
	update(cfg: PlaygroundCfg, opts: { fps: number; once: boolean }): void {
		this.cfg = cfg;
		this.fps = Math.max(1, Math.min(30, opts.fps));
		this.once = opts.once;
		this.frame = 0;
		this.lastTick = null;
		this.stop();

		if (this.once || cfg.mode === 'solid') {
			try {
				this.target.innerHTML = this.wasm.renderOnce(cfg);
			} catch (e) {
				this.renderError(e);
			}
			return;
		}

		this.start();
	}

	/** Stop the animation loop. Safe to call when not running. */
	stop(): void {
		if (this.rafId) this.caf(this.rafId);
		this.rafId = 0;
		this.running = false;
		this.generation += 1;
	}

	private start(): void {
		this.running = true;
		const tickMs = 1000 / this.fps;
		const myGen = this.generation;
		const loop = (t: number): void => {
			if (myGen !== this.generation || !this.running || !this.cfg) return;
			if (this.lastTick === null || t - this.lastTick >= tickMs) {
				try {
					this.target.innerHTML = this.wasm.renderFrame(this.cfg, this.frame);
				} catch (e) {
					this.renderError(e);
					this.stop();
					return;
				}
				this.frame += 1;
				this.lastTick = t;
			}
			this.rafId = this.raf(loop);
		};
		this.rafId = this.raf(loop);
	}

	private renderError(e: unknown): void {
		const msg = e instanceof Error ? e.message : String(e);
		this.target.textContent = `render error: ${msg}`;
	}

	/** Exposed for tests. */
	get currentFrame(): number {
		return this.frame;
	}

	get isRunning(): boolean {
		return this.running;
	}
}
