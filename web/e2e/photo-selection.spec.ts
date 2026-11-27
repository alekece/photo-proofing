import { expect, test, type Locator } from '@playwright/test';

for (const touch of [false, true]) {
	test.describe(touch ? 'phone touch' : 'desktop keyboard', () => {
		test.use({
			viewport: touch ? { width: 390, height: 844 } : { width: 1440, height: 900 },
			isMobile: touch,
			hasTouch: touch
		});

		test('keeps all favorites and the checked subset independently downloadable', async ({
			page
		}) => {
			test.skip(!process.env.PP_E2E_WORKSPACE_FIXTURE, 'Requires the synthetic workspace fixture.');
			const activate = (control: Locator) => (touch ? control.tap() : control.press('Enter'));
			const button = (name: string) => page.getByRole('button', { name, exact: true });
			const requests: (string | null)[] = [];
			await page.route('**/gallery-2/export', (route) => {
				if (route.request().method() === 'GET') return route.fulfill({ status: 204 });
				requests.push(route.request().postData());
				return route.fulfill({
					json: {
						id: `zip-${requests.length}`,
						status: { kind: 'ready' },
						photo_count: requests.length === 1 ? 4 : 2,
						created_at: '2026-11-18T10:00:00Z'
					}
				});
			});
			await page.route('**/gallery-2/export/*/download', (route) =>
				route.fulfill({ contentType: 'application/octet-stream', body: 'synthetic attachment' })
			);
			const mounted = page.waitForRequest('**/api/v1/events');
			await page.goto('/dashboard/gallery-2');
			await mounted;
			await activate(button('Select Coast-1.jpg'));
			await activate(button('Delete'));
			await activate(page.getByRole('dialog').getByRole('button', { name: 'Cancel' }));
			await expect(page.getByRole('dialog')).toBeHidden();
			await activate(button('Exit selection'));
			await expect(button('Open Coast-1.jpg')).toBeVisible();
			await activate(page.getByRole('tab', { name: /Favorites/ }));
			await activate(button('Open Coast-1.jpg'));
			await expect(page.getByRole('dialog').getByRole('button', { name: /download/i })).toHaveCount(
				0
			);
			await activate(button('Close viewer'));
			await expect(page.getByRole('dialog')).toBeHidden();
			await activate(button('Select Coast-1.jpg'));
			const all = button('Download all');
			await expect(all).toBeInViewport();
			await expect(button('Download selected (1)')).toBeInViewport();
			await Promise.all([page.waitForEvent('download'), activate(all)]);
			expect(requests).toEqual([null]);
			await activate(button('Select Coast-3.jpg'));
			await Promise.all([page.waitForEvent('download'), activate(button('Download selected (2)'))]);
			expect(JSON.parse(requests[1]!)).toEqual({ photo_ids: ['p1', 'p3'] });
			await activate(button('Select Coast-1.jpg'));
			await activate(button('Select Coast-3.jpg'));
			await expect(button('Download selected (0)')).toHaveCount(0);
			await expect(button('Exit selection')).toHaveCount(0);
			await expect(all).toBeEnabled();
			await expect(button('Open Coast-1.jpg')).toBeVisible();
		});
	});
}
