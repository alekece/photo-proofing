import { describe, expect, it } from 'vitest';
import { exportStatusSchema, parseOrNull } from '#lib/gallery/schemas.js';
import { apiErrorMessage, readErrorCode } from './errors.js';
import { useLocale } from './testing.js';

describe('apiErrorMessage in English', () => {
	useLocale('en');

	it('prefers the code, then the status, then a generic sentence', () => {
		expect(apiErrorMessage(507)).toBe('Photo storage is full. Free some space and try again.');
		expect(apiErrorMessage(422, 'invalid_studio_name')).toBe(
			'Enter a studio name of up to 100 characters.'
		);
		expect(apiErrorMessage(409, 'zip_not_ready')).toBe(
			'The ZIP isn’t ready yet. Try again in a moment.'
		);
		expect(apiErrorMessage(409, 'contact_email_taken')).toBe(
			'A contact with this email already exists.'
		);
		expect(apiErrorMessage(401, 'gallery_locked')).toBe(
			'Enter the 6-digit access code to open this gallery.'
		);
		expect(apiErrorMessage(401, 'invalid_access_code')).toBe(
			'Incorrect access code. Check the 6 digits and try again.'
		);
		expect(apiErrorMessage(409, 'access_code_locked')).toBe(
			'The access code can’t be changed once the gallery is shared.'
		);
		expect(apiErrorMessage(503, 'access_code_unavailable')).toBe(
			'Access codes aren’t configured on this server.'
		);
		expect(apiErrorMessage(422, 'unknown')).toBe(apiErrorMessage(422));
		expect(apiErrorMessage(409, 'constructor')).toBe(apiErrorMessage(409));
		expect(apiErrorMessage(418)).toBe('Something went wrong. Please try again.');
	});
});

describe('apiErrorMessage in French', () => {
	useLocale('fr');

	it('explains each refusal in French', () => {
		expect(apiErrorMessage(507)).toBe(
			'Le stockage des photos est plein. Libérez de l’espace et réessayez.'
		);
		expect(apiErrorMessage(409, 'zip_in_progress')).toBe(
			'Un ZIP est déjà en préparation. Attendez qu’il soit terminé.'
		);
		expect(apiErrorMessage(409, 'contact_email_taken')).toBe(
			'Un contact utilise déjà cette adresse e-mail.'
		);
		expect(apiErrorMessage(401, 'invalid_access_code')).toBe(
			'Code d’accès incorrect. Vérifiez les 6 chiffres et réessayez.'
		);
		expect(apiErrorMessage(409, 'access_code_locked')).toBe(
			'Le code d’accès ne peut plus être modifié une fois la galerie partagée.'
		);
		expect(apiErrorMessage(503, 'access_code_unavailable')).toBe(
			'Les codes d’accès ne sont pas configurés sur ce serveur.'
		);
		expect(apiErrorMessage(418)).toBe('Une erreur s’est produite. Veuillez réessayer.');
	});
});

describe('readErrorCode', () => {
	it('reads a string code and ignores anything else', () => {
		expect(readErrorCode({ code: 'busy' })).toBe('busy');
		expect(readErrorCode({ code: 3 })).toBeUndefined();
		expect(readErrorCode(null)).toBeUndefined();
		expect(readErrorCode('busy')).toBeUndefined();
	});
});

describe('a failed export status', () => {
	it('accepts any string code, so a new server code never breaks the page', () => {
		expect(parseOrNull(exportStatusSchema, { kind: 'failed', code: 'from_the_future' })).toEqual({
			kind: 'failed',
			code: 'from_the_future'
		});
		expect(parseOrNull(exportStatusSchema, { kind: 'failed' })).toBeNull();
	});
});
