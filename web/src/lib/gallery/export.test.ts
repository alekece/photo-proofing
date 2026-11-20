import { expect, it, vi } from 'vitest';
import { exportUrl, latestExport, requestExport } from './export';

it.each([
	['photos', '', ''],
	['finals', '?finals=true', '?finals=true']
] as const)(
	'uses the owner %s routes for POST, status and download',
	async (set, query, download) => {
		const ready = {
			id: 'e/1',
			status: { kind: 'ready' },
			photo_count: 1,
			created_at: '2026-11-07T10:00:00Z'
		};
		const fetcher = vi.fn<typeof fetch>(async () => Response.json(ready));
		const path = `/api/v1/galleries/g%2F1/export${query}`;

		await requestExport('g/1', fetcher, ['p1'], set);
		await latestExport('g/1', fetcher, set);

		expect(fetcher.mock.calls).toEqual([
			[path, expect.objectContaining({ method: 'POST', body: '{"photo_ids":["p1"]}' })],
			[path, expect.objectContaining({ method: 'GET' })]
		]);
		expect(exportUrl('g/1', 'e/1', set)).toBe(
			`/api/v1/galleries/g%2F1/export/e%2F1/download${download}`
		);
	}
);
