import { expect, test } from '@playwright/test';

test('gallery return restores the grid, but browser Back still honors Activity', async ({
	page
}) => {
	test.skip(
		!process.env.PP_E2E_WORKSPACE_FIXTURE,
		'Use a disposable workspace with the Coast galleries and activity fixture.'
	);
	await page.route('**/api/v1/notifications', (route) =>
		route.fulfill({
			json: {
				unread: 1,
				notifications: [
					{
						id: 'notification-1',
						gallery_id: 'gallery-28',
						title: 'Forest 28',
						hero_id: null,
						kind: 'open',
						count: 1,
						selected: 0,
						updated_at: '2026-11-18T10:00:00Z',
						read_at: null
					}
				]
			}
		})
	);
	await page.setViewportSize({ width: 1200, height: 660 });
	await page.goto('/dashboard?sort=title');
	const search = page.getByRole('searchbox', { name: 'Search galleries' });
	const status = page.getByRole('button', { name: /status/i });
	const draft = page.getByRole('checkbox', { name: /draft/i });
	await expect(async () => {
		if (!(await draft.isVisible())) await status.click();
		await expect(draft).toBeVisible({ timeout: 1250 });
	}).toPass({ timeout: 12000 });
	await draft.uncheck();
	await status.click();
	await search.fill('Coast');
	expect(await page.getByRole('banner').count()).toBe(1);
	const gallery = page.getByRole('link', { name: /coast 21/i });
	await gallery.scrollIntoViewIfNeeded();
	const position = await page.evaluate(() => window.scrollY);
	expect(position).toBeGreaterThan(0);

	for (const useBrowserBack of [true, false]) {
		await gallery.click();
		await expect(page.getByRole('link', { name: 'Back to galleries' })).toBeVisible();
		if (useBrowserBack) await page.goBack();
		else await page.getByRole('link', { name: 'Back to galleries' }).click();
		await expect(page).toHaveURL(/\/dashboard\?sort=title$/);
		await expect(search).toHaveValue('Coast');
		await expect(page.getByRole('link', { name: /coast 01|forest 28/i })).toHaveCount(0);
		await expect.poll(() => page.evaluate(() => window.scrollY)).toBe(position);
	}

	await gallery.click();
	await page.getByRole('button', { name: 'Notifications, 1 unread' }).click();
	await expect(page.getByRole('button', { name: 'Mark all as read' })).toBeInViewport({
		ratio: 1
	});
	await page.getByRole('link', { name: /forest 28/i }).click();
	await expect(page).toHaveURL(/\/dashboard\/gallery-28$/);
	await page.goBack();
	await expect(page).toHaveURL(/\/dashboard\/gallery-21$/);
	await page.getByRole('link', { name: 'Back to galleries' }).click();
	await expect(search).toHaveValue('Coast');
	await expect.poll(() => page.evaluate(() => window.scrollY)).toBe(position);

	await page.getByRole('link', { name: 'Activity', exact: true }).click();
	await page.getByRole('link', { name: 'Coast 01' }).first().click();
	await expect(page.getByRole('link', { name: 'Back to galleries' })).toBeVisible();
	await page.goBack();
	await expect(page).toHaveURL(/\/activity$/);
});

test('all unread notifications fit within reach on short viewports', async ({ page }) => {
	test.skip(!process.env.PP_E2E_WORKSPACE_FIXTURE, 'Run against the synthetic workspace fixture.');
	await page.route('**/api/v1/notifications', (route) =>
		route.fulfill({
			json: {
				unread: 12,
				notifications: Array.from({ length: 12 }, (_, i) => ({
					id: `notification-${i}`,
					gallery_id: `gallery-${i + 1}`,
					title: `Unread gallery ${i + 1}`,
					hero_id: null,
					kind: 'open',
					count: 1,
					selected: 0,
					updated_at: '2026-11-18T10:00:00Z',
					read_at: null
				}))
			}
		})
	);

	for (const width of [900, 390]) {
		await page.setViewportSize({ width, height: 390 });
		await page.goto('/dashboard');
		await page.getByRole('button', { name: 'Notifications, 12 unread' }).click();
		const last = page.getByRole('link', { name: /unread gallery 12/i });
		await last.focus();
		await expect(last).toBeInViewport({ ratio: 1 });
		await expect(page.getByRole('button', { name: 'Mark all as read' })).toBeInViewport({
			ratio: 1
		});
	}
});

test('the gallery form closes back to its trigger, including with reduced motion', async ({
	page
}) => {
	test.skip(!process.env.PP_E2E_WORKSPACE_FIXTURE, 'Run against the synthetic workspace fixture.');
	await page.goto('/dashboard');
	const trigger = page.getByRole('button', { name: '+ New gallery', exact: true });
	const form = page.getByRole('dialog', { name: 'New gallery' });

	for (const reducedMotion of ['no-preference', 'reduce'] as const) {
		await page.emulateMedia({ reducedMotion });
		await expect(async () => {
			if (!(await form.isVisible())) await trigger.click({ timeout: 1000 });
			await expect(form).toBeVisible({ timeout: 1000 });
		}).toPass();
		await expect(form.getByRole('textbox').first()).toBeFocused();
		await form.getByRole('button', { name: 'Cancel' }).click();
		await expect(form).toBeHidden();
		await expect(trigger).toBeFocused();
	}
});

test('mobile client viewer steps on a horizontal swipe but not a vertical drag', async ({
	browser
}) => {
	test.skip(!process.env.PP_E2E_WORKSPACE_FIXTURE, 'Run against the synthetic workspace fixture.');
	const context = await browser.newContext({
		viewport: { width: 390, height: 844 },
		isMobile: true,
		hasTouch: true
	});

	try {
		const page = await context.newPage();
		await page.goto('/g/synthetic-link-fixture');
		const photo = page.getByRole('button', { name: /open.*1 of 7/i });
		const viewer = page.getByRole('dialog');
		await expect(async () => {
			if (!(await viewer.isVisible())) await photo.click();
			await expect(viewer).toBeVisible({ timeout: 1000 });
		}).toPass();
		await expect(viewer.getByText('1 of 7', { exact: true })).toBeVisible();
		await expect(viewer.getByRole('button', { name: 'Next proof' })).toBeVisible();

		const device = await context.newCDPSession(page);
		const swipe = async (x: number, y: number) => {
			await device.send('Input.dispatchTouchEvent', {
				type: 'touchStart',
				touchPoints: [{ x: 255, y: 380 }]
			});
			await device.send('Input.dispatchTouchEvent', {
				type: 'touchMove',
				touchPoints: [{ x, y }]
			});
			await device.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
		};

		await swipe(252, 500);
		await expect(viewer.getByText('1 of 7', { exact: true })).toBeVisible();
		await swipe(95, 384);
		await expect(viewer.getByText('2 of 7', { exact: true })).toBeVisible();
	} finally {
		await context.close();
	}
});
