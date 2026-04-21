// Faux block caret for a text input. The native caret is hidden via
// `caret-color: transparent`; this overlay tracks selectionEnd and renders
// a blinking block at that column. Works in any monospace input.

export function mountBlockCaret(input: HTMLInputElement): void {
	const parent = input.parentElement;
	if (!parent) return;

	const caret = document.createElement('span');
	caret.className = 'block-caret';
	caret.setAttribute('aria-hidden', 'true');
	parent.appendChild(caret);

	const measure = document.createElement('span');
	measure.style.position = 'absolute';
	measure.style.visibility = 'hidden';
	measure.style.whiteSpace = 'pre';
	measure.style.pointerEvents = 'none';
	measure.style.top = '-9999px';
	document.body.appendChild(measure);

	const update = (): void => {
		const cs = getComputedStyle(input);
		measure.style.fontFamily = cs.fontFamily;
		measure.style.fontSize = cs.fontSize;
		measure.style.fontWeight = cs.fontWeight;
		measure.style.fontStyle = cs.fontStyle;
		measure.style.letterSpacing = cs.letterSpacing;
		measure.style.fontVariantLigatures = cs.fontVariantLigatures;

		const pos = input.selectionEnd ?? input.value.length;
		measure.textContent = input.value.slice(0, pos) || '';
		const w = measure.getBoundingClientRect().width;

		const inputRect = input.getBoundingClientRect();
		const parentRect = parent.getBoundingClientRect();

		caret.style.left = `${inputRect.left - parentRect.left + w}px`;
		caret.style.top = `${inputRect.top - parentRect.top}px`;
		caret.style.height = `${inputRect.height}px`;
		caret.style.fontSize = cs.fontSize;
	};

	const events = ['input', 'keyup', 'keydown', 'click', 'focus', 'select'];
	for (const ev of events) input.addEventListener(ev, () => requestAnimationFrame(update));
	document.addEventListener('selectionchange', () => {
		if (document.activeElement === input) requestAnimationFrame(update);
	});
	window.addEventListener('resize', update);

	input.addEventListener('focus', () => caret.classList.add('on'));
	input.addEventListener('blur', () => caret.classList.remove('on'));

	if (document.activeElement === input) caret.classList.add('on');
	update();
}
