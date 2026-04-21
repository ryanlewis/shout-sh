// FontPicker — keyboard-navigable list. Arrow keys move focus, Enter/click
// selects. Fires onChange(fontName) whenever the selection changes.

export const FONTS = [
	'block',
	'slick',
	'tiny',
	'grid',
	'pallet',
	'shade',
	'chrome',
	'simple',
	'simpleblock',
	'3d',
	'simple3d',
	'huge',
	'console',
] as const;

export type FontName = (typeof FONTS)[number];

export function mountFontPicker(
	root: HTMLElement,
	initial: FontName,
	onChange: (font: FontName) => void,
): void {
	root.innerHTML = '';
	let current: FontName = initial;

	const buttons: HTMLButtonElement[] = FONTS.map((name, idx) => {
		const btn = document.createElement('button');
		btn.type = 'button';
		btn.setAttribute('role', 'radio');
		btn.setAttribute('data-font', name);
		btn.setAttribute('tabindex', idx === 0 ? '0' : '-1');
		btn.textContent = name;
		btn.addEventListener('click', () => select(name));
		btn.addEventListener('keydown', (e) => onKey(e, idx));
		root.appendChild(btn);
		return btn;
	});

	const select = (name: FontName): void => {
		if (current === name) return;
		current = name;
		for (const btn of buttons) {
			const isSel = btn.getAttribute('data-font') === name;
			btn.setAttribute('aria-checked', String(isSel));
			btn.setAttribute('tabindex', isSel ? '0' : '-1');
		}
		onChange(name);
	};

	const onKey = (e: KeyboardEvent, idx: number): void => {
		let next: number | null = null;
		if (e.key === 'ArrowRight' || e.key === 'ArrowDown') next = (idx + 1) % FONTS.length;
		else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp')
			next = (idx - 1 + FONTS.length) % FONTS.length;
		else if (e.key === 'Home') next = 0;
		else if (e.key === 'End') next = FONTS.length - 1;
		else if (e.key === 'Enter' || e.key === ' ') {
			const name = FONTS[idx];
			if (name) select(name);
			e.preventDefault();
			return;
		}
		if (next !== null) {
			e.preventDefault();
			const target = buttons[next];
			const name = FONTS[next];
			if (target && name) {
				target.focus();
				select(name);
			}
		}
	};

	// Paint initial state.
	for (const btn of buttons) {
		const isSel = btn.getAttribute('data-font') === initial;
		btn.setAttribute('aria-checked', String(isSel));
	}
}
