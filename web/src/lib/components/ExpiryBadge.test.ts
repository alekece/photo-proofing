import { render } from '@testing-library/svelte';
import { afterEach, describe, expect, it } from 'vitest';
import ExpiryBadge from './ExpiryBadge.svelte';

const zone = process.env.TZ;
afterEach(() => {
	process.env.TZ = zone;
});

describe('ExpiryBadge', () => {
	it('shows the exact calendar day in time zones behind and ahead of UTC', () => {
		for (const tz of ['America/New_York', 'Pacific/Auckland']) {
			process.env.TZ = tz;
			const { container, unmount } = render(ExpiryBadge, {
				expiresAt: '2026-10-31'
			});
			expect(container.textContent?.trim()).toBe('Open until October 31, 2026');
			expect(container.querySelector('time')?.getAttribute('datetime')).toBe('2026-10-31');
			unmount();
		}
	});
});
