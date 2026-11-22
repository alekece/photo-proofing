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
	last_opened_at: null,
	shoot_date: '2026-11-16',
	states: [i === 0 ? 'draft' : i === 6 ? 'delivered' : 'shared'],
	created_at: now,
	updated_at: now,
	photo_count: 7,
	favorite_count: i % 8,
	final_count: 0,
	downloaded_count: 0
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
const activity = ['open', 'favorite', 'comment'].map((kind, i) => ({
	id: `activity-${i}`,
	kind,
	gallery_id: 'gallery-1',
	gallery_title: 'Coast 01',
	contact: { id: contact.id, name: contact.name },
	detail: null,
	photo_ids: kind === 'favorite' ? photos.map(({ id }) => id) : [],
	count: kind === 'favorite' ? photos.length : 1,
	first_at: now,
	last_at: now
}));
const comments = [
	{
		id: 'c1',
		photo_id: 'p2',
		author: 'client',
		body: 'Would a warmer edit work here?',
		created_at: now,
		edited_at: null,
		deleted_at: null
	}
];
const palette = [
	['#d5a792', '#52665f'],
	['#dcc5a9', '#836359'],
	['#b2b9a2', '#67594d'],
	['#d2b999', '#8a5750'],
	['#b0c1c3', '#5f6a79']
];

createServer((req, res) => {
	const { pathname: path, searchParams } = new URL(req.url, 'http://localhost');
	const json = (body, status = 200) =>
		res.writeHead(status, { 'Content-Type': 'application/json' }).end(JSON.stringify(body));

	if (path === '/api/v1/events') {
		res.writeHead(200, { 'Content-Type': 'text/event-stream', 'Cache-Control': 'no-store' });
		res.write(': fixture\n\n');
		return;
	}
	if (path === '/api/v1/auth/renew') return res.writeHead(204).end();
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
	if (path.endsWith('/timeline')) {
		return json({ events: path === '/api/v1/timeline' ? activity : activity.slice(1), next: null });
	}
	if (path.endsWith('/contacts')) return json([contact]);
	if (path.endsWith('/limits'))
		return json({ concurrency: 2, max_size: 10485760, max_pixels: 18000000 });
	if (path.endsWith('/comments')) {
		return json(
			path.includes('/photos/')
				? comments
				: [{ photo_id: 'p2', count: 1, latest_body: comments[0].body }]
		);
	}
	const gallery = galleries.find(({ id }) => path.endsWith(`/${id}`));
	if (gallery) return json(gallery);
	if (path.endsWith('/favorites')) return json(photos.slice(0, 4).map(({ id }) => id));
	if (path === '/api/v1/photos') {
		return json(
			photos.map((p, i) => ({
				...p,
				gallery_id: i % 2 ? 'gallery-2' : 'gallery-1',
				gallery_title: i % 2 ? 'Coast 02' : 'Coast 01',
				kind: i % 2 ? 'final' : 'proof',
				revision: 1
			}))
		);
	}
	if (path.endsWith('/photos')) return json(searchParams.get('finals') === 'true' ? [] : photos);
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
