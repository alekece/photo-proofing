import type { Contact } from './api.js';

/** Up to two uppercase monogram letters from the contact's name, skipping connectors like `&`. */
export function contactInitials(name: string): string {
	const words = name
		.trim()
		.split(/\s+/)
		.map((part) => Array.from(part).find((ch) => /\p{L}|\p{N}/u.test(ch)))
		.filter((ch): ch is string => ch !== undefined);

	if (words.length === 0) return '?';
	if (words.length === 1) return words[0].toUpperCase();

	return `${words[0]}${words[words.length - 1]}`.toUpperCase();
}

/** Alphabetical order matching the backend (`lower(name), id`). */
export function sortContacts(list: readonly Contact[]): Contact[] {
	return [...list].sort((a, b) => {
		const byName = a.name.toLowerCase().localeCompare(b.name.toLowerCase());

		return byName !== 0 ? byName : a.id.localeCompare(b.id);
	});
}
