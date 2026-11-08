import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import Placeholder from './Placeholder.svelte';

describe('Placeholder', () => {
	it('is decorative without a label and has no icon by default', () => {
		const { container } = render(Placeholder, { props: { background: 'sheet' } });
		expect(container.firstElementChild?.getAttribute('aria-hidden')).toBe('true');
		expect(container.querySelector('svg')).toBeNull();
	});

	it('shows the sheet variant with icon and label', () => {
		const { container } = render(Placeholder, {
			props: { background: 'sheet', icon: true, label: 'Your cover photo will appear here' }
		});
		expect(container.firstElementChild?.getAttribute('data-background')).toBe('sheet');
		expect(container.firstElementChild?.getAttribute('aria-hidden')).toBeNull();
		expect(screen.getByText('Your cover photo will appear here')).toBeTruthy();
		expect(screen.getByTestId('placeholder-icon')).toBeTruthy();
	});

	it('pulses without a label while loading', () => {
		const { container } = render(Placeholder, {
			props: { background: 'sheet', loading: true, label: 'ignored', icon: true }
		});
		expect(container.firstElementChild?.className).toMatch(/pp-breathe/);
		expect(container.textContent?.trim()).toBe('');
		expect(container.firstElementChild?.getAttribute('aria-hidden')).toBe('true');
	});
});
