import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { mountShouter } from '../src/ui/Shouter.js';
import type { UrlState } from '../src/urls.js';

const initial: Omit<UrlState, 'text'> = {
	font: 'block',
	mode: 'rainbow',
	color: '',
	preset: '',
	once: false,
	fps: 10,
	letterSpacing: 1,
	maxLength: 0,
	padding: 2,
	background: '',
};

describe('Shouter', () => {
	beforeEach(() => {
		vi.useFakeTimers();
	});
	afterEach(() => {
		vi.useRealTimers();
	});

	it('fires initial state synchronously (via microtask)', async () => {
		const input = document.createElement('input');
		input.value = 'HI';
		const onChange = vi.fn();
		mountShouter({ input, initial, onChange, debounceMs: 150 });

		await Promise.resolve();
		expect(onChange).toHaveBeenCalledTimes(1);
		expect(onChange.mock.calls[0]?.[0].text).toBe('HI');
	});

	it('debounces input events', async () => {
		const input = document.createElement('input');
		input.value = 'HI';
		const onChange = vi.fn();
		mountShouter({ input, initial, onChange, debounceMs: 150 });
		await Promise.resolve(); // flush the initial microtask
		onChange.mockClear();

		input.value = 'H';
		input.dispatchEvent(new Event('input'));
		input.value = 'HE';
		input.dispatchEvent(new Event('input'));
		input.value = 'HEL';
		input.dispatchEvent(new Event('input'));

		vi.advanceTimersByTime(149);
		expect(onChange).not.toHaveBeenCalled();

		vi.advanceTimersByTime(1);
		expect(onChange).toHaveBeenCalledTimes(1);
		expect(onChange.mock.calls[0]?.[0].text).toBe('HEL');
	});

	it('control setters fire state updates immediately', async () => {
		const input = document.createElement('input');
		input.value = 'HI';
		const onChange = vi.fn();
		const s = mountShouter({ input, initial, onChange });
		await Promise.resolve();
		onChange.mockClear();

		s.setMode('fire');
		expect(onChange).toHaveBeenCalledTimes(1);
		expect(onChange.mock.calls[0]?.[0].mode).toBe('fire');

		s.setFont('tiny');
		expect(onChange.mock.calls[1]?.[0].font).toBe('tiny');

		s.setOnce(true);
		expect(onChange.mock.calls[2]?.[0].once).toBe(true);

		s.setFps(20);
		expect(onChange.mock.calls[3]?.[0].fps).toBe(20);

		s.setPreset('sunset');
		expect(onChange.mock.calls[4]?.[0].preset).toBe('sunset');
	});
});
