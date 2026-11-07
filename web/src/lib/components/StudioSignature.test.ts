import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import StudioSignature from './StudioSignature.svelte';

describe('StudioSignature', () => {
	it('prints nothing without a studio or logo', () => {
		render(StudioSignature, {});

		expect(screen.queryByTestId('studio-signature')).toBeNull();
	});

	it('prints the name by default', () => {
		render(StudioSignature, { studio: 'Studio Lune', logo: '/logo.png' });

		expect(screen.getByText('Studio Lune')).toBeTruthy();
		expect(document.querySelector('img')).toBeNull();
	});

	it('prints the logo and the name for both', () => {
		render(StudioSignature, { studio: 'Studio Lune', logo: '/logo.png', signature: 'both' });

		expect(screen.getByText('Studio Lune')).toBeTruthy();
		expect(document.querySelector('img')?.getAttribute('src')).toBe('/logo.png');
		expect(screen.getByTestId('studio-signature').className).toContain('flex-col');
	});

	it('falls back to the name when a logo-only signature has no logo', () => {
		render(StudioSignature, { studio: 'Studio Lune', signature: 'logo' });

		expect(screen.getByText('Studio Lune')).toBeTruthy();
	});

	it('prints nothing for none', () => {
		render(StudioSignature, { studio: 'Studio Lune', logo: '/logo.png', signature: 'none' });

		expect(screen.queryByTestId('studio-signature')).toBeNull();
	});
});
