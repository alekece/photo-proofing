import { describe, expect, it } from 'vitest';
import { apiErrorMessage } from './errors.js';
import { useLocale } from './testing.js';

describe('apiErrorMessage in English', () => {
	useLocale('en');

	it('explains each refusal without the API text', () => {
		expect(apiErrorMessage(507)).toBe('Photo storage is full. Free some space and try again.');
		expect(apiErrorMessage(422, 'studio_name')).toBe(
			'Enter a studio name of up to 100 characters.'
		);
		expect(apiErrorMessage(422, 'unknown')).toBe(apiErrorMessage(422));
		expect(apiErrorMessage(418)).toBe('Something went wrong. Please try again.');
	});
});

describe('apiErrorMessage in French', () => {
	useLocale('fr');

	it('explains each refusal in French', () => {
		expect(apiErrorMessage(507)).toBe(
			'Le stockage des photos est plein. Libérez de l’espace et réessayez.'
		);
		expect(apiErrorMessage(418)).toBe('Une erreur s’est produite. Veuillez réessayer.');
	});
});
