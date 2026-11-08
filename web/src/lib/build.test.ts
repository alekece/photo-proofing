import { expect, it } from 'vitest';
import { displayVersion } from './build.js';

it('shortens a commit SHA and leaves a release version alone', () => {
	expect(displayVersion('0123456789abcdef0123456789abcdef01234567')).toBe('0123456');
	expect(displayVersion('1.4.0')).toBe('1.4.0');
});
