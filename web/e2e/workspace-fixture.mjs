import { createServer } from 'node:http';

const now = '2026-11-18T10:00:00Z';
const galleries = Array.from({ length: 28 }, (_, i) => ({
	id: `gallery-${i + 1}`,
	title:
		i === 2
			? 'Coast 03 — Une collection de portraits au bord de la mer au soleil couchant'
			: `${i === 27 ? 'Forest' : 'Coast'} ${String(i + 1).padStart(2, '0')}`,
	favorite: i === 1,
	layout: 'masonry',
	cover_layout: 'full_bleed',
	focal_point: { x: 50, y: 35 },
	heading_font: 'editorial',
	grid_density: 'compact',
	grid_spacing: 'compact',
	expires_at: null,
	included_photos: 4,
	watermark_enabled: false,
	hero_id: i === 3 ? null : 'p1',
	share_token: i === 0 ? null : 'synthetic-link-fixture',
	shared_at: i === 0 ? null : now,
	delivered_at: i === 6 ? now : null,
	last_opened_at: null,
	shoot_date: '2026-11-16',
	states: [i === 0 ? 'draft' : i === 6 ? 'delivered' : 'shared'],
	created_at: now,
	updated_at: now,
	photo_count: 7,
	favorite_count: i % 8,
	final_count: i === 6 ? 3 : 0,
	downloaded_count: i === 6 ? 2 : 0
}));
const settings = {
	studio_name: 'Studio Côte',
	has_logo: false,
	mark: 'name',
	pattern: 'once',
	position: 8,
	size: 4,
	signature: 'name',
	signature_size: 4,
	signature_color: '#F6CCB4',
	proof_quality: 'medium'
};
const photos = Array.from({ length: 7 }, (_, i) => ({
	id: `p${i + 1}`,
	original_name: `Coast-${i + 1}.jpg`,
	size: 2000,
	width: i % 3 === 0 ? 640 : 900,
	height: i % 3 === 0 ? 830 : 600,
	status: 'ready',
	favorited_at: i < 4 ? now : null,
	created_at: now
}));
const finalPhotos = Array.from({ length: 3 }, (_, i) => ({
	...photos[i],
	id: `p${i + 8}`,
	original_name: `Coast-${i + 1}-retouched.jpg`,
	favorited_at: null,
	downloaded_at: i < 2 ? `2026-11-18T1${i + 1}:00:00Z` : null,
	revision: 1
}));
const contact = {
	id: 'synthetic-contact',
	name: 'Camille Lune',
	email: 'camille@example.test',
	phone: null,
	notes: 'Prefers a warmer edit.',
	galleries: [{ id: 'gallery-1', title: 'Coast 01' }],
	created_at: now,
	updated_at: now
};
const contacts = [
	contact,
	{
		...contact,
		id: 'synthetic-contact-2',
		name: 'Élise Brume',
		email: 'elise@example.test',
		phone: null,
		notes: 'Prefers matte prints.',
		galleries: []
	},
	{
		...contact,
		id: 'synthetic-contact-3',
		name: 'Hugo Rivage',
		email: 'hugo@example.test',
		phone: '+1 202 555 0143',
		notes: null,
		galleries: [
			{ id: 'gallery-3', title: 'Coast 03' },
			{ id: 'gallery-4', title: 'Coast 04' }
		]
	},
	{
		...contact,
		id: 'synthetic-contact-4',
		name: 'Inès et Malo Sablé',
		email: 'ines-malo@example.test',
		phone: null,
		notes: 'Prefer black and white portraits.',
		galleries: []
	},
	{
		...contact,
		id: 'synthetic-contact-5',
		name: 'Jade Vallon',
		email: 'jade@example.test',
		phone: null,
		notes: 'Reply in the spring.',
		galleries: [{ id: 'gallery-5', title: 'Coast 05' }]
	},
	{
		...contact,
		id: 'synthetic-contact-6',
		name: 'Lou Dune',
		email: 'lou@example.test',
		phone: null,
		notes: null,
		galleries: []
	}
];
const linkedContactIds = new Set([contact.id]);
const activityNow = new Date(Date.now() - 3_600_000).toISOString();
const activity = ['open', 'favorite', 'comment'].map((kind, i) => ({
	id: `activity-${i}`,
	kind,
	gallery_id: 'gallery-1',
	gallery_title: 'Coast 01',
	contact: { id: contact.id, name: contact.name },
	detail: null,
	photo_ids: kind === 'favorite' ? Array.from({ length: 12 }, (_, i) => `p${i + 1}`) : [],
	count: kind === 'favorite' ? 12 : 1,
	first_at: activityNow,
	last_at: activityNow
}));
const historicalAt = new Date(Date.now() - 3 * 86_400_000).toISOString();
activity.push({
	...activity[0],
	id: 'activity-history',
	kind: 'download',
	gallery_id: galleries[20].id,
	gallery_title: galleries[20].title,
	contact: { id: 'synthetic-visitor', name: 'Élise Brume' },
	first_at: historicalAt,
	last_at: historicalAt
});

function activityPage(path, params) {
	const gallery = path.match(/\/galleries\/([^/]+)\/timeline$/)?.[1];
	const fold = (text) => text.normalize('NFD').replace(/\p{M}/gu, '').toLowerCase();
	const query = fold(params.get('q')?.trim() ?? '');
	const kind = params.get('kind');
	const range = { today: 1, '7d': 7, '30d': 30 }[params.get('range')];
	const oldestFirst = params.get('sort') === 'oldest';
	const offset = Number(params.get('cursor') ?? 0);
	const end = offset + 3;
	const rows = activity
		.filter((event) => {
			if (gallery && event.gallery_id !== gallery) return false;
			if (kind && event.kind !== kind) return false;
			if (range && Date.parse(event.last_at) < Date.now() - range * 86_400_000) return false;

			return (
				!query ||
				(!gallery && fold(event.gallery_title).includes(query)) ||
				fold(event.contact.name).includes(query)
			);
		})
		.sort((a, b) => {
			const order = a.last_at.localeCompare(b.last_at) || a.id.localeCompare(b.id);

			return oldestFirst ? order : -order;
		});

	return { events: rows.slice(offset, end), next: rows.length > end ? String(end) : null };
}
const comments = [
	{
		id: 'c1',
		photo_id: 'p2',
		author: 'client',
		body: 'Would a warmer edit work here?',
		created_at: '2026-11-18T09:10:00Z',
		edited_at: null,
		deleted_at: null,
		editable: false,
		contact: { id: contact.id, name: contact.name }
	},
	{
		id: 'c2',
		photo_id: 'p2',
		author: 'photographer',
		body: 'I can soften the highlights and keep the natural skin tones.',
		created_at: '2026-11-18T09:18:00Z',
		edited_at: null,
		deleted_at: null,
		editable: true
	},
	{
		id: 'c3',
		photo_id: 'p2',
		author: 'client',
		body: 'Perfect — the second crop is the one we prefer.',
		created_at: '2026-11-18T09:26:00Z',
		edited_at: null,
		deleted_at: null,
		editable: false,
		contact: { id: contact.id, name: contact.name }
	},
	{
		id: 'c4',
		photo_id: 'p2',
		author: 'client',
		body: 'Could you keep this version available in color?',
		created_at: '2026-11-18T09:34:00Z',
		edited_at: null,
		deleted_at: null,
		editable: false
	},
	{
		id: 'c5',
		photo_id: 'p2',
		author: 'photographer',
		body: 'Absolutely. I’ll include it in the final selection.',
		created_at: '2026-11-18T09:41:00Z',
		edited_at: '2026-11-18T09:43:00Z',
		deleted_at: null,
		editable: true
	}
];
const palette = [
	['#d5a792', '#52665f'],
	['#dcc5a9', '#836359'],
	['#b2b9a2', '#67594d'],
	['#d2b999', '#8a5750'],
	['#b0c1c3', '#5f6a79']
];

createServer(async (req, res) => {
	const { pathname: path, searchParams } = new URL(req.url, 'http://localhost');
	const json = (body, status = 200) =>
		res.writeHead(status, { 'Content-Type': 'application/json' }).end(JSON.stringify(body));

	if (path === '/api/v1/events') {
		res.writeHead(200, { 'Content-Type': 'text/event-stream', 'Cache-Control': 'no-store' });
		res.write(': fixture\n\n');
		return;
	}
	if (path === '/api/v1/auth/renew') return res.writeHead(204).end();
	if (path === '/api/v1/contacts' && req.method === 'POST') {
		let body = '';
		for await (const chunk of req) body += chunk;
		const input = JSON.parse(body);
		const created = {
			id: `synthetic-contact-${Date.now()}`,
			name: input.name,
			email: input.email,
			phone: input.phone ?? null,
			notes: input.notes ?? null,
			galleries: [],
			created_at: now,
			updated_at: now
		};
		contacts.push(created);
		return json(created, 201);
	}
	const galleryContact = path.match(/^\/api\/v1\/galleries\/[^/]+\/contacts\/([^/]+)$/);
	if (galleryContact && req.method === 'PUT') {
		const item = contacts.find(({ id }) => id === galleryContact[1]);
		if (!item) return json({}, 404);
		linkedContactIds.add(item.id);
		return json(item);
	}
	if (galleryContact && req.method === 'DELETE') {
		if (!linkedContactIds.delete(galleryContact[1])) return res.writeHead(404).end();
		return res.writeHead(204).end();
	}
	if (req.method !== 'GET') return json({}, 405);
	if (path === '/api/v1/auth/status') {
		return json({
			identity: { account_id: 'synthetic-account', workspace_id: 'synthetic-workspace' },
			setup_available: false,
			idle_timeout_ms: 3600000
		});
	}
	if (path === '/api/v1/settings') return json(settings);
	if (path === '/api/v1/galleries') return json(galleries);
	if (path === '/api/v1/galleries/shared/synthetic-link-fixture') {
		return json({
			...galleries[1],
			locked: false,
			delivered: false,
			studio_name: settings.studio_name,
			signature: settings.signature,
			signature_size: settings.signature_size,
			signature_color: settings.signature_color,
			has_logo: false
		});
	}
	if (path.endsWith('/notifications')) return json({ notifications: [], unread: 0 });
	if (path.endsWith('/timeline')) return json(activityPage(path, searchParams));
	if (path === '/api/v1/contacts') return json(contacts);
	if (path.endsWith('/contacts'))
		return json(
			contacts
				.filter(({ id }) => linkedContactIds.has(id))
				.map((item, index) => ({ ...item, link_id: `synthetic-gallery-link-${index}` }))
		);
	if (path.endsWith('/limits'))
		return json({ concurrency: 2, max_size: 10485760, max_pixels: 18000000 });
	if (path.endsWith('/comments')) {
		return json(
			path.includes('/photos/')
				? comments
				: [{ photo_id: 'p2', count: comments.length, latest_body: comments.at(-1).body }]
		);
	}
	const gallery = galleries.find(({ id }) => path.endsWith(`/${id}`));
	if (gallery) return json(gallery);
	if (path.endsWith('/favorites')) return json(photos.slice(0, 4).map(({ id }) => id));
	if (path === '/api/v1/photos') {
		return json([
			...photos.map((p, i) => ({
				...p,
				gallery_id: i % 2 ? 'gallery-2' : 'gallery-1',
				gallery_title: i % 2 ? 'Coast 02' : 'Coast 01',
				kind: 'proof',
				revision: 1
			})),
			...finalPhotos.map((p) => ({
				...p,
				gallery_id: 'gallery-7',
				gallery_title: 'Coast 07',
				kind: 'final',
				revision: 1
			})),
			...galleries.slice(2).map((g, i) => ({
				...photos[0],
				id: `library-${i}`,
				gallery_id: g.id,
				gallery_title: g.title,
				kind: 'proof',
				revision: 1
			}))
		]);
	}
	if (path.endsWith('/photos'))
		return json(searchParams.get('finals') === 'true' ? finalPhotos : photos);
	if (/thumbnail|preview|\/cover$/.test(path)) {
		const key =
			Number(path.match(/gallery-(\d+)/)?.[1] ?? 2) + Number(path.match(/p(\d+)/)?.[1] ?? 1);
		const [sky, hills] = palette[key % palette.length];
		return res
			.writeHead(200, { 'Content-Type': 'image/svg+xml' })
			.end(
				`<svg xmlns="http://www.w3.org/2000/svg" width="900" height="650" viewBox="0 0 900 650"><rect width="900" height="650" fill="${sky}"/><circle cx="${230 + (key % 6) * 70}" cy="180" r="95" fill="#ffffdd" opacity=".58"/><path d="M0 400 C180 310 235 240 410 385 Q660 220 900 365 V650 H0Z" fill="${hills}"/><path d="M0 500 Q325 365 620 502 T900 460 V650 H0Z" fill="#25272b" opacity=".55"/></svg>`
			);
	}
	return json({}, 404);
}).listen(Number(process.env.PP_WORKSPACE_FIXTURE_PORT ?? 15274), '127.0.0.1');
