import { describe, expect, it } from 'vitest';
import { resolveApiAddress } from './api-address';

const production = { dev: false, building: false };

describe('resolveApiAddress', () => {
	it('keeps the configured address without a trailing slash', () => {
		expect(resolveApiAddress('http://api:8080/', production)).toBe('http://api:8080');
	});

	it('falls back to the local default in development', () => {
		expect(resolveApiAddress(undefined, { dev: true, building: false })).toBe(
			'http://127.0.0.1:8080'
		);
	});

	it('accepts a missing address while building', () => {
		expect(resolveApiAddress(undefined, { dev: false, building: true })).toBe('');
	});

	it.each([undefined, ''])('names the variable when it is %j in production', (value) => {
		expect(() => resolveApiAddress(value, production)).toThrow('PP_API_INTERNAL_URL');
	});
});
