// Thin wrapper over window.umami.track — no-ops when the script is blocked
// or hasn't loaded yet.

type UmamiGlobal = { track?: (name: string, data?: Record<string, unknown>) => void };

export function track(name: string, data?: Record<string, unknown>): void {
	const u = (window as unknown as { umami?: UmamiGlobal }).umami;
	try {
		u?.track?.(name, data);
	} catch {
		// Analytics must never break the page.
	}
}
