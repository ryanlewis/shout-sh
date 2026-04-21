import { describe, it, expect, vi } from 'vitest';
import { Preview } from '../src/ui/Preview.js';
import type { Wasm, PlaygroundCfg } from '../src/wasm.js';

function mockWasm(): Wasm & { once: ReturnType<typeof vi.fn>; frame: ReturnType<typeof vi.fn> } {
	const once = vi.fn((cfg: PlaygroundCfg) => `once:${cfg.text}`);
	const frame = vi.fn((cfg: PlaygroundCfg, n: number) => `frame:${cfg.text}:${n}`);
	return { renderOnce: once, renderFrame: frame, once, frame };
}

function makeRaf() {
	const cbs: FrameRequestCallback[] = [];
	let nextId = 1;
	const raf = (cb: FrameRequestCallback) => {
		cbs.push(cb);
		return nextId++;
	};
	const caf = vi.fn();
	const tick = (t: number) => {
		const pending = cbs.splice(0);
		for (const cb of pending) cb(t);
	};
	return { raf, caf, tick, pending: () => cbs.length };
}

const cfg: PlaygroundCfg = {
	text: 'HI',
	font: 'block',
	mode: 'rainbow',
	color: '',
	preset: '',
};

describe('Preview', () => {
	it('once mode renders a single frame and does not loop', () => {
		const wasm = mockWasm();
		const target = document.createElement('pre');
		const { raf, caf, pending } = makeRaf();

		const p = new Preview({ wasm, target, raf, caf });
		p.update(cfg, { fps: 10, once: true });

		expect(wasm.once).toHaveBeenCalledTimes(1);
		expect(wasm.frame).not.toHaveBeenCalled();
		expect(target.innerHTML).toBe('once:HI');
		expect(pending()).toBe(0);
		expect(p.isRunning).toBe(false);
	});

	it('solid mode renders once (solid is naturally static)', () => {
		const wasm = mockWasm();
		const target = document.createElement('pre');
		const { raf, caf } = makeRaf();

		const p = new Preview({ wasm, target, raf, caf });
		p.update({ ...cfg, mode: 'solid' }, { fps: 10, once: false });

		expect(wasm.once).toHaveBeenCalledTimes(1);
		expect(wasm.frame).not.toHaveBeenCalled();
	});

	it('animated modes drive renderFrame via RAF and throttle by fps', () => {
		const wasm = mockWasm();
		const target = document.createElement('pre');
		const { raf, caf, tick } = makeRaf();

		const p = new Preview({ wasm, target, raf, caf });
		p.update(cfg, { fps: 10, once: false });

		// 10 fps → 100ms per frame.
		tick(0); // first tick always renders (lastTick===0)
		expect(wasm.frame).toHaveBeenCalledWith(cfg, 0);
		expect(p.currentFrame).toBe(1);

		tick(50); // too soon, skipped
		expect(wasm.frame).toHaveBeenCalledTimes(1);

		tick(100); // due
		expect(wasm.frame).toHaveBeenCalledTimes(2);
		expect(wasm.frame).toHaveBeenLastCalledWith(cfg, 1);
		expect(target.innerHTML).toBe('frame:HI:1');
	});

	it('update resets the frame counter and cancels the prior loop', () => {
		const wasm = mockWasm();
		const target = document.createElement('pre');
		const { raf, caf, tick } = makeRaf();

		const p = new Preview({ wasm, target, raf, caf });
		p.update(cfg, { fps: 10, once: false });
		tick(0);
		tick(100);
		expect(p.currentFrame).toBe(2);

		p.update({ ...cfg, text: 'WORLD' }, { fps: 10, once: false });
		expect(caf).toHaveBeenCalled();
		expect(p.currentFrame).toBe(0);
		tick(0);
		expect(wasm.frame).toHaveBeenLastCalledWith({ ...cfg, text: 'WORLD' }, 0);
	});

	it('stop cancels the pending RAF', () => {
		const wasm = mockWasm();
		const target = document.createElement('pre');
		const { raf, caf } = makeRaf();

		const p = new Preview({ wasm, target, raf, caf });
		p.update(cfg, { fps: 10, once: false });
		p.stop();
		expect(caf).toHaveBeenCalled();
		expect(p.isRunning).toBe(false);
	});

	it('wasm render error is surfaced in the DOM', () => {
		const wasm: Wasm = {
			renderOnce: () => {
				throw new Error('boom');
			},
			renderFrame: () => '',
		};
		const target = document.createElement('pre');
		const { raf, caf } = makeRaf();

		const p = new Preview({ wasm, target, raf, caf });
		p.update(cfg, { fps: 10, once: true });
		expect(target.textContent).toContain('render error');
		expect(target.textContent).toContain('boom');
	});
});
