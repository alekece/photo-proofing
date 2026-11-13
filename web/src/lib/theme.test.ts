import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

const css = readFileSync(resolve(process.cwd(), 'src/app.css'), 'utf8');

function token(name: string): string {
	const match = new RegExp(`${name}:\\s*(#[0-9a-fA-F]{6})`).exec(css);
	if (!match) throw new Error(`missing token ${name}`);

	return match[1];
}

function luminance(hex: string): number {
	const [r, g, b] = [1, 3, 5]
		.map((start) => parseInt(hex.slice(start, start + 2), 16) / 255)
		.map((c) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));

	return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function contrast(foreground: string, background: string): number {
	const [light, dark] = [luminance(foreground), luminance(background)].sort((a, b) => b - a);

	return (light + 0.05) / (dark + 0.05);
}

describe('warm paper tokens', () => {
	it('uses the approved page color', () => {
		expect(token('--background').toLowerCase()).toBe('#f4efe8');
	});

	it.each(['--foreground', '--muted-foreground', '--danger'])(
		'%s is readable on the page and panel backgrounds',
		(name) => {
			expect(contrast(token(name), token('--background'))).toBeGreaterThanOrEqual(4.5);
			expect(contrast(token(name), token('--panel'))).toBeGreaterThanOrEqual(4.5);
		}
	);

	it('self-hosts gallery variable fonts without external font CDNs', () => {
		expect(css).toContain("@import '@fontsource-variable/cormorant-garamond';");
		expect(css).toContain("@import '@fontsource-variable/playfair-display';");
		expect(css).toContain("@import '@fontsource-variable/plus-jakarta-sans';");
		expect(css).not.toMatch(/fonts\.googleapis\.com|fonts\.gstatic\.com/i);
	});
});
