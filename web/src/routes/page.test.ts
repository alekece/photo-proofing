import { isRedirect } from '@sveltejs/kit';
import { describe, expect, it } from 'vitest';
import { load } from './+page.server.js';

describe('root page', () => {
	it('sends visitors to the dashboard, which guards sign-in and setup', () => {
		let thrown: unknown;

		try {
			load({} as never);
		} catch (error) {
			thrown = error;
		}

		expect(isRedirect(thrown) && thrown.location).toBe('/dashboard');
	});
});
