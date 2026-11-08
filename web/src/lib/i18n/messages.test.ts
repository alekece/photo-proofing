import { describe, expect, it } from 'vitest';
import en from '../../../messages/en.json';
import fr from '../../../messages/fr.json';

/** Keys whose French reads the same as the English, on purpose. */
const SAME_IN_BOTH: string[] = ['header_notifications', 'summary_photos'];

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
