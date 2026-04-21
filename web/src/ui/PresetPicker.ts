// PresetPicker — a chips group for palette presets. Empty string means
// "no preset" (falls back to bare color / cfonts defaults).

// Keep in sync with shout-core/src/presets.rs::PRESETS. The server is the
// source of truth at runtime; this list only drives the chips UI.
export const PRESETS = [
	'sunset',
	'ocean',
	'mint',
	'candy',
	'matrix',
	'mono',
	'neon',
	'ember',
] as const;

export type PresetName = (typeof PRESETS)[number] | '';

export interface PresetPickerHandle {
	setDisabled(disabled: boolean): void;
}

export function mountPresetPicker(
	root: HTMLElement,
	initial: PresetName,
	onChange: (preset: PresetName) => void,
): PresetPickerHandle {
	root.innerHTML = '';
	let current: PresetName = initial;

	const entries: { name: PresetName; label: string }[] = [
		{ name: '', label: 'none' },
		...PRESETS.map((p) => ({ name: p as PresetName, label: p })),
	];

	const buttons: HTMLButtonElement[] = entries.map((entry, idx) => {
		const btn = document.createElement('button');
		btn.type = 'button';
		btn.setAttribute('role', 'radio');
		btn.setAttribute('data-preset', entry.name);
		btn.setAttribute('tabindex', idx === 0 ? '0' : '-1');
		btn.textContent = entry.label;
		btn.addEventListener('click', () => select(entry.name));
		btn.addEventListener('keydown', (e) => onKey(e, idx));
		root.appendChild(btn);
		return btn;
	});

	const select = (name: PresetName): void => {
		if (current === name) return;
		current = name;
		for (const btn of buttons) {
			const isSel = btn.getAttribute('data-preset') === name;
			btn.setAttribute('aria-checked', String(isSel));
			btn.setAttribute('tabindex', isSel ? '0' : '-1');
		}
		onChange(name);
	};

	const onKey = (e: KeyboardEvent, idx: number): void => {
		let next: number | null = null;
		if (e.key === 'ArrowRight' || e.key === 'ArrowDown') next = (idx + 1) % entries.length;
		else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp')
			next = (idx - 1 + entries.length) % entries.length;
		else if (e.key === 'Home') next = 0;
		else if (e.key === 'End') next = entries.length - 1;
		else if (e.key === 'Enter' || e.key === ' ') {
			const entry = entries[idx];
			if (entry) select(entry.name);
			e.preventDefault();
			return;
		}
		if (next !== null) {
			e.preventDefault();
			const target = buttons[next];
			const entry = entries[next];
			if (target && entry) {
				target.focus();
				select(entry.name);
			}
		}
	};

	for (const btn of buttons) {
		const isSel = btn.getAttribute('data-preset') === initial;
		btn.setAttribute('aria-checked', String(isSel));
	}

	return {
		setDisabled(disabled) {
			root.classList.toggle('chips--disabled', disabled);
			root.setAttribute('aria-disabled', String(disabled));
			for (const btn of buttons) {
				btn.disabled = disabled;
				// Re-derive roving tabindex from `current` so disable→enable
				// restores keyboard reachability on the selected chip.
				const isSel = btn.getAttribute('data-preset') === current;
				btn.setAttribute('tabindex', disabled ? '-1' : isSel ? '0' : '-1');
			}
		},
	};
}
