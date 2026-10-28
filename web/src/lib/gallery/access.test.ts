import { describe, expect, it } from 'vitest';
import { toClientView } from './access';
import type { GalleryRecord } from './types';

const now = new Date('2026-06-01T12:00:00Z');
const proofs = [{ id: 'p1', alt: 'Proof 1', previewUrl: 'data:image/svg+xml,p1' }];
const record = (over: Partial<GalleryRecord> = {}): GalleryRecord => ({
	title: 'Spring session',
	state: 'proofing',
	expiresAt: '2026-07-01T00:00:00Z',
	proofs,
	...over
});

describe('toClientView', () => {
	it.each(['proofing', 'selection_received', 'retouching'] as const)(
		'exposes proof previews in %s',
		(state) => {
			const view = toClientView(record({ state }), now);
			expect(view.access).toBe('granted');
			if (view.access === 'granted') expect(view.proofs).toEqual(proofs);
		}
	);

	it('exposes no proofs in delivery', () => {
		const view = toClientView(record({ state: 'delivery' }), now);
		expect(view.access).toBe('granted');
		if (view.access === 'granted') expect(view.proofs).toEqual([]);
	});

	it.each([
		['expired state', record({ state: 'expired' })],
		['past expiry date', record({ expiresAt: '2026-05-01T00:00:00Z' })],
		['draft', record({ state: 'draft' })],
		['unknown gallery', null]
	])('is a bare unavailable view for %s', (_name, rec) => {
		expect(toClientView(rec, now)).toEqual({ access: 'unavailable' });
	});
});
