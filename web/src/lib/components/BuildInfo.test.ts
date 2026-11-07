import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import BuildInfo from './BuildInfo.svelte';

const sha = '0123456789abcdef0123456789abcdef01234567';

describe('frontend build identity', () => {
	it('shows the short revision and copies the complete revision', async () => {
		const user = userEvent.setup();
		const copy = vi.spyOn(navigator.clipboard, 'writeText').mockResolvedValue();
		render(BuildInfo, { sha });

		expect(screen.getByText('Build 0123456')).toBeTruthy();
		await user.click(screen.getByRole('button', { name: 'Copy build SHA' }));
		expect(copy).toHaveBeenCalledWith(sha);
		expect(screen.getByRole('status').textContent).toBe('Copied');
	});

	it('offers manual copying when clipboard access fails', async () => {
		const user = userEvent.setup();
		vi.spyOn(navigator.clipboard, 'writeText').mockRejectedValue(new Error('Denied'));
		render(BuildInfo, { sha });

		await user.click(screen.getByRole('button', { name: 'Copy build SHA' }));
		const input = screen.getByRole('textbox', { name: 'Full build SHA' });
		expect(input).toHaveProperty('value', sha);
		expect(document.activeElement).toBe(input);
		expect(screen.getByRole('status').textContent).toContain('copy it manually');
	});

	it('labels an unstamped local build without a copy action', () => {
		render(BuildInfo, { sha: '' });
		expect(screen.getByText('Development build')).toBeTruthy();
		expect(screen.queryByRole('button')).toBeNull();
	});
});
