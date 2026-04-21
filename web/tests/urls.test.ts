import { describe, it, expect } from 'vitest';
import { buildPath, buildCurl, type UrlState } from '../src/urls.js';

const base: UrlState = {
	text: 'HELLO',
	font: 'block',
	mode: 'solid',
	color: '',
	once: false,
	fps: 10,
};

describe('buildPath', () => {
	it('plain text uses default font implicitly', () => {
		expect(buildPath({ ...base })).toBe('/HELLO');
	});

	it('non-default font becomes a directive', () => {
		expect(buildPath({ ...base, font: 'tiny' })).toBe('/tiny/HELLO');
	});

	it('rainbow + once compose into path directives', () => {
		expect(buildPath({ ...base, mode: 'rainbow', once: true })).toBe('/rainbow+once/HELLO');
	});

	it('fire + tiny orders directives font-first', () => {
		expect(buildPath({ ...base, font: 'tiny', mode: 'fire' })).toBe('/tiny+fire/HELLO');
	});

	it('spaces are encoded as +', () => {
		expect(buildPath({ ...base, text: 'hello world' })).toBe('/hello+world');
	});

	it('special characters are percent-encoded', () => {
		expect(buildPath({ ...base, text: 'a/b?c' })).toBe('/a%2Fb%3Fc');
	});

	it('non-default fps adds a query param', () => {
		expect(buildPath({ ...base, mode: 'rainbow', fps: 20 })).toBe('/rainbow/HELLO?fps=20');
	});

	it('default fps is omitted', () => {
		expect(buildPath({ ...base, mode: 'rainbow', fps: 10 })).toBe('/rainbow/HELLO');
	});

	it('solid mode alone produces no directive segment', () => {
		expect(buildPath({ ...base, text: 'hi', mode: 'solid' })).toBe('/hi');
	});
});

describe('buildCurl', () => {
	it('renders the canonical curl command for animated state', () => {
		const s: UrlState = { ...base, mode: 'rainbow' };
		expect(buildCurl(s)).toBe('curl shout.sh/rainbow/HELLO');
	});

	it('renders the canonical curl command for once state', () => {
		const s: UrlState = { ...base, mode: 'rainbow', once: true };
		expect(buildCurl(s)).toBe('curl shout.sh/rainbow+once/HELLO');
	});
});
