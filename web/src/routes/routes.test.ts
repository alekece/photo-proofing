import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import Dashboard from './dashboard/+page.svelte';
import Gallery from './g/[gallery]/+page.svelte';

describe('route shells', () => {
	it('dashboard renders its heading', () => {
		render(Dashboard);
		expect(screen.getByRole('heading', { name: 'Dashboard' })).toBeTruthy();
	});

	it('gallery renders its heading and the slug as escaped text', () => {
		const slug = 'a b<script>';
		const { container } = render(Gallery, { props: { data: { gallery: slug } } });
		expect(screen.getByRole('heading', { name: 'Gallery' })).toBeTruthy();
		expect(container.textContent).toContain(slug);
		expect(container.querySelector('script')).toBeNull();
	});
});
