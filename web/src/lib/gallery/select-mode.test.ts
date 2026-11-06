import { describe, expect, it } from 'vitest';
import { SelectMode } from './select-mode.svelte.js';

describe('SelectMode', () => {
	it('toggles ids and counts them', () => {
		const mode = new SelectMode();

		mode.toggle('a');
		mode.toggle('b');
		mode.toggle('a');

		expect(mode.count).toBe(1);
		expect(mode.has('b')).toBe(true);
	});

	it('selects only the ids it is given', () => {
		const mode = new SelectMode();

		mode.selectAll(['a', 'b']);

		expect(mode.count).toBe(2);
	});

	it('forgets the selection on leaving', () => {
		const mode = new SelectMode();

		mode.enter();
		mode.selectAll(['a']);
		mode.leave();

		expect(mode.active).toBe(false);
		expect(mode.count).toBe(0);
	});
});
