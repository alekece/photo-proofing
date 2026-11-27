import { afterEach, describe, expect, it, vi } from 'vitest';
import { rails } from './rails.svelte.js';

function screen(narrow: boolean) {
	vi.stubGlobal('matchMedia', () => ({
		matches: narrow,
		addEventListener: () => {},
		removeEventListener: () => {}
	}));
	window.matchMedia = globalThis.matchMedia;
}

afterEach(() => {
	localStorage.clear();
	delete document.documentElement.dataset.workspaceRail;
	vi.unstubAllGlobals();
});

describe('rails', () => {
	it('collapses on a narrow screen until the photographer chooses', () => {
		screen(true);
		const stop = rails.start();

		expect(rails.collapsed).toBe(true);

		rails.toggle();

		expect(rails.collapsed).toBe(false);
		expect(document.documentElement.dataset.workspaceRail).toBe('expanded');
		stop();

		screen(true);
		const restart = rails.start();

		expect(rails.collapsed).toBe(false);

		restart();
	});

	it('keeps a collapse chosen on a wide screen', () => {
		screen(false);
		const stop = rails.start();

		expect(rails.collapsed).toBe(false);

		rails.toggle();
		stop();

		const restart = rails.start();

		expect(rails.collapsed).toBe(true);

		restart();
	});
});
