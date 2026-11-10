import { describe, expect, it } from 'vitest';
import { m } from '#lib/paraglide/messages.js';
import { useLocale } from './testing.js';
import en from '../../../messages/en.json';
import fr from '../../../messages/fr.json';

/** Keys whose French reads the same as the English, on purpose. */
const SAME_IN_BOTH: string[] = [
	'comments_client',
	'common_logo',
	'common_photos',
	'header_notifications',
	'summary_photos',
	'watermark_position',
	'watermark_position_cell'
];

const texts = (catalogue: Record<string, unknown>) =>
	Object.fromEntries(Object.entries(catalogue).filter(([key]) => key !== '$schema'));

describe('message catalogues', () => {
	it('translate every message', () => {
		expect(Object.keys(texts(fr)).sort()).toEqual(Object.keys(texts(en)).sort());
	});

	it('leave no French message empty', () => {
		const empty = Object.entries(texts(fr)).filter(([, text]) => JSON.stringify(text) === '""');

		expect(empty).toEqual([]);
	});

	it('only keep the English text where it is deliberate', () => {
		const english = texts(en);
		const same = Object.entries(texts(fr))
			.filter(([key, text]) => JSON.stringify(text) === JSON.stringify(english[key]))
			.map(([key]) => key);

		expect(same.sort()).toEqual([...SAME_IN_BOTH].sort());
	});
});

describe('French counts', () => {
	useLocale('fr');

	it('agree with a single deleted photo', () => {
		expect(m.photos_partly_deleted({ deleted: 1, total: 3 })).toBe(
			'1 photo sur 3 supprimée. Réessayez.'
		);
		expect(m.photos_partly_deleted({ deleted: 2, total: 3 })).toBe(
			'2 photos sur 3 supprimées. Réessayez.'
		);
	});

	it('say "retouchées", not "finales", for the delivered photos', () => {
		const stale = Object.entries(texts(fr)).filter(([, text]) =>
			/finale/i.test(JSON.stringify(text))
		);

		expect(stale).toEqual([]);
	});
});
