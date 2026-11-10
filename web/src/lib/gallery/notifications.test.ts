import { describe, expect, it } from 'vitest';
import { notificationBadge, notificationText } from './notifications.svelte';
import type { Notification } from './schemas';
import { useLocale } from '#lib/i18n/testing.js';

const comment: Notification = {
	id: 'n1',
	gallery_id: 'g1',
	title: 'Wedding',
	hero_id: null,
	kind: 'comment',
	count: 2,
	selected: 0,
	updated_at: '2026-11-10T09:00:00Z',
	read_at: null
};
describe('English comment notifications', () => {
	useLocale('en');
	it('names the activity with singular and plural counts', () => {
		expect(notificationText(comment)).toBe('2 new comments');
		expect(notificationText({ ...comment, count: 1 })).toBe('1 new comment');
		expect(notificationBadge(comment)).toBe('Comments');
	});
});
describe('French comment notifications', () => {
	useLocale('fr');
	it('names the activity with singular and plural counts', () => {
		expect(notificationText(comment)).toBe('2 nouveaux commentaires');
		expect(notificationText({ ...comment, count: 1 })).toBe('1 nouveau commentaire');
		expect(notificationBadge(comment)).toBe('Commentaires');
	});
});
