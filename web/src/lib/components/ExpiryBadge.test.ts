import { render } from '@testing-library/svelte';
import { afterEach, describe, expect, it } from 'vitest';
import { endOfLocalDay } from '#lib/gallery/format.js';
import ExpiryBadge from './ExpiryBadge.svelte';

const zone = process.env.TZ;
afterEach(() => {
	process.env.TZ = zone;
});

describe('ExpiryBadge', () => {
	it('shows the day the photographer picked in a time zone behind UTC', () => {
		process.env.TZ = 'America/New_York';
		const { container } = render(ExpiryBadge, {
			expiresAt: endOfLocalDay('2026-10-31'),
			now: new Date('2026-10-01T00:00:00Z')
		});
		expect(container.textContent).toMatch(/October 31, 2026/);
	});
});
