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
	const shared = page.getByRole('menuitemradio', { name: /shared/i });
	await expect(async () => {
		if (!(await shared.isVisible())) await status.click();
		await expect(shared).toBeVisible({ timeout: 1250 });
	}).toPass({ timeout: 12000 });
	await shared.click();
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

test('activity uses the available width for favorite thumbnails', async ({ page }) => {
	test.skip(!process.env.PP_E2E_WORKSPACE_FIXTURE, 'Run against the synthetic workspace fixture.');
	await page.setViewportSize({ width: 1200, height: 800 });
	await page.goto('/activity');
	const photos = page.getByRole('img', { name: 'Photo', exact: true });
	await expect(photos).toHaveCount(7);
	const rows = await photos.evaluateAll((images) =>
		images.map((image) => image.getBoundingClientRect().top)
	);
	expect(new Set(rows).size).toBe(1);
});

test('workspace and Settings navigation share transparent resting and distinct hover states', async ({
	page
}) => {
	test.skip(!process.env.PP_E2E_WORKSPACE_FIXTURE, 'Run against the synthetic workspace fixture.');
	await page.setViewportSize({ width: 1200, height: 800 });
	await page.goto('/settings');
	const rail = page.getByRole('banner');
	const contacts = rail.getByRole('link', { name: 'Contacts' });
	const settings = rail.getByRole('link', { name: 'Settings' });
	const general = page.getByRole('tab', { name: 'General' });
	const watermark = page.getByRole('tab', { name: 'Watermark' });
	for (const item of [contacts, watermark]) {
		await expect(item).toHaveCSS('background-color', 'rgba(0, 0, 0, 0)');
		await item.hover();
		await expect(item).toHaveCSS('background-color', 'rgb(224, 214, 200)');
	}
	for (const item of [settings, general]) {
		await item.hover();
		await expect(item).toHaveCSS('background-color', 'rgb(43, 36, 32)');
	}
	await expect(rail.getByRole('button', { name: /^Language:/ })).toHaveCount(0);
	await expect(page.getByRole('radio', { name: 'English', checked: true })).toBeVisible();
});

test('Settings aligns its actions with the heading and exposes both languages', async ({
	page
}) => {
	test.skip(!process.env.PP_E2E_WORKSPACE_FIXTURE, 'Run against the synthetic workspace fixture.');
	await page.setViewportSize({ width: 1280, height: 800 });
	await page.goto('/settings');
	const heading = page.getByRole('heading', { name: 'Settings', exact: true });
	const save = page.getByRole('button', { name: 'Save', exact: true });
	const cancel = page.getByRole('button', { name: 'Cancel', exact: true });
	await expect(save).toBeDisabled();
	await expect(cancel).toBeDisabled();
	const title = (await heading.boundingBox())!;
	const action = (await save.boundingBox())!;
	expect(action.x).toBeGreaterThan(title.x + title.width);
	expect(Math.abs(action.y + action.height / 2 - title.y - title.height / 2)).toBeLessThan(4);
	for (const width of [1280, 390]) {
		await page.setViewportSize({ width, height: 800 });
		for (const name of ['English', 'Français']) {
			await expect(page.getByRole('radio', { name })).toBeInViewport({ ratio: 1 });
		}
		await expect(save).toBeInViewport({ ratio: 1 });
	}
});

test('the sticky Settings header does not cover focused controls on a short phone', async ({
	page
}) => {
	test.skip(!process.env.PP_E2E_WORKSPACE_FIXTURE, 'Run against the synthetic workspace fixture.');
	await page.setViewportSize({ width: 390, height: 520 });
	await page.goto('/settings');
	const watermark = page.getByRole('tab', { name: 'Watermark' });
	await expect(async () => {
		await watermark.click();
		await expect(watermark).toHaveAttribute('aria-selected', 'true', { timeout: 500 });
	}).toPass({ timeout: 5000 });
	const last = page.getByRole('button', { name: 'Position 9', exact: true });
	await watermark.focus();
	for (
		let i = 0;
		i < 20 && !(await last.evaluate((button) => button === document.activeElement));
		i++
	) {
		await page.keyboard.press('Tab');
	}
	await expect(last).toBeFocused();
	for (let i = 0; i < 13; i++) await page.keyboard.press('Shift+Tab');

	const once = page.getByRole('button', { name: 'Once', exact: true });
	const save = page.getByRole('button', { name: 'Save', exact: true });
	await expect(once).toBeFocused();
	const control = (await once.boundingBox())!;
	const action = (await save.boundingBox())!;
	expect(control.y).toBeGreaterThan(action.y + action.height);
});

test('a failed keyboard language change returns focus for another attempt', async ({ page }) => {
	test.skip(!process.env.PP_E2E_WORKSPACE_FIXTURE, 'Run against the synthetic workspace fixture.');
	let requests = 0;
	await page.route('**/api/v1/settings/language', (route) => {
		requests++;
		return route.fulfill({ status: 500 });
	});
	await page.goto('/settings');
	const english = page.getByRole('radio', { name: 'English' });
	await expect(async () => {
		await english.press('ArrowRight');
		await expect(page.getByText('The language was not saved. Please try again.')).toBeVisible({
			timeout: 500
		});
	}).toPass({ timeout: 5000 });
	await expect(english).toBeChecked();
	await expect(english).toBeFocused();
	const first = requests;
	await page.keyboard.press('ArrowRight');
	await expect.poll(() => requests).toBe(first + 1);
});

test('the locked access-code switch stays beside its label on a phone', async ({ page }) => {
	test.skip(!process.env.PP_E2E_WORKSPACE_FIXTURE, 'Run against the synthetic workspace fixture.');
	await page.setViewportSize({ width: 390, height: 844 });
	await page.goto('/dashboard/gallery-2');
	const toggle = page.getByRole('switch', { name: 'Require access code' });
	await expect(async () => {
		await page.getByRole('tab', { name: 'Settings', exact: true }).click();
		await expect(toggle).toBeVisible({ timeout: 500 });
	}).toPass({ timeout: 5000 });
	await expect(toggle).toBeDisabled();
	await expect(toggle).toHaveAccessibleDescription('Cannot add a code after sharing.');
	const label = (await page.getByText('Require access code', { exact: true }).boundingBox())!;
	const control = (await toggle.boundingBox())!;
	expect(control.y).toBeLessThan(label.y + label.height);
	expect(label.y).toBeLessThan(control.y + control.height);
});

test('photo filters are directly reachable and fit the phone toolbar', async ({ page }) => {
	test.skip(!process.env.PP_E2E_WORKSPACE_FIXTURE, 'Run against the synthetic workspace fixture.');
	await page.setViewportSize({ width: 390, height: 844 });
	const hydrated = page.waitForRequest(
		(request) => new URL(request.url()).pathname === '/api/v1/events'
	);
	await page.goto('/photos');
	await hydrated;
	const gallery = page.getByRole('button', { name: /^Gallery:/ });
	const type = page.getByRole('button', { name: /^Type:/ });
	await expect(gallery).toBeInViewport({ ratio: 1 });
	await expect(type).toBeInViewport({ ratio: 1 });
	const visible = page.getByRole('list', { name: 'Photos' }).getByRole('listitem');
	await gallery.click();
	await page.getByRole('menuitemradio', { name: 'Coast 02' }).click();
	await expect(visible).toHaveCount(3);
	await type.click();
	await page.getByRole('menuitemradio', { name: 'Proof' }).click();
	await expect(page.getByText('No photos match these filters.')).toBeVisible();
	await type.click();
	await page.getByRole('menuitemradio', { name: 'Final' }).click();
	await expect(visible).toHaveCount(3);
	await expect(visible.first()).toContainText('Coast 02');
});

test('a long gallery title leaves status and selection visible on a phone', async ({ page }) => {
	test.skip(!process.env.PP_E2E_WORKSPACE_FIXTURE, 'Run against the synthetic workspace fixture.');
	await page.setViewportSize({ width: 390, height: 844 });
	await page.goto('/dashboard');
	const card = page.getByRole('link', { name: /Coast 03 —/ });
	const chip = card.getByText('Shared');
	const bounds = (await card.boundingBox())!;
	const status = (await chip.boundingBox())!;
	expect(status.y).toBeGreaterThanOrEqual(bounds.y);
	const select = card.locator('..').getByRole('button');
	const target = (await select.boundingBox())!;
	expect(status.y).toBeGreaterThanOrEqual(target.y + target.height);
});

test('a gallery underneath the sticky search field cannot intercept its click', async ({
	page
}) => {
	test.skip(!process.env.PP_E2E_WORKSPACE_FIXTURE, 'Run against the synthetic workspace fixture.');
	await page.setViewportSize({ width: 1280, height: 640 });
	const mounted = page.waitForRequest((r) => new URL(r.url()).pathname === '/api/v1/events');
	await page.goto('/dashboard');
	await mounted;
	const disc = page.locator('main li button[aria-pressed]').nth(12);
	await disc.scrollIntoViewIfNeeded();
	const search = page.getByRole('searchbox');
	const field = (await search.boundingBox())!;
	const target = (await disc.boundingBox())!;
	const y = field.y + field.height / 2;
	await page.evaluate((by) => window.scrollBy(0, by), target.y + target.height / 2 - y);
	await page.mouse.click(target.x + target.width / 2 + 5, y);
	await expect(search).toBeFocused();
});

test('the last gallery filter is reachable inside a scrollable phone menu', async ({ page }) => {
	test.skip(!process.env.PP_E2E_WORKSPACE_FIXTURE, 'Run against the synthetic workspace fixture.');
	await page.setViewportSize({ width: 390, height: 600 });
	const mounted = page.waitForRequest((r) => new URL(r.url()).pathname === '/api/v1/events');
	await page.goto('/photos');
	await mounted;
	const trigger = page.getByRole('button', { name: /^Gallery:/ });
	await trigger.click();
	const menu = page.getByRole('menu');
	const last = menu.getByRole('menuitemradio', { name: 'Forest 28' });
	expect(await menu.evaluate((el) => el.scrollHeight > el.clientHeight)).toBe(true);
	await page.keyboard.press('End');
	await expect(last).toBeFocused();
	await expect(last).toBeInViewport({ ratio: 1 });
	await last.click();
	await expect(trigger).toContainText('Forest 28');
});

test.describe('short touch viewport', () => {
	test.use({ hasTouch: true, viewport: { width: 390, height: 480 } });
	test('an upward-opening gallery menu accepts taps over the workspace header', async ({
		page
	}) => {
		test.skip(
			!process.env.PP_E2E_WORKSPACE_FIXTURE,
			'Run against the synthetic workspace fixture.'
		);
		const mounted = page.waitForRequest((r) => new URL(r.url()).pathname === '/api/v1/events');
		await page.goto('/photos');
		await mounted;
		const trigger = page.getByRole('button', { name: /^Gallery:/ });
		await trigger.tap();
		const all = page.getByRole('menuitemradio', { name: 'All galleries' });
		const first = (await all.boundingBox())!;
		expect(first.y).toBeLessThan((await trigger.boundingBox())!.y);
		await all.tap({ timeout: 3000 });
		await expect(trigger).toHaveAttribute('aria-expanded', 'false');
	});
});

test.describe('French selection actions', () => {
	test.use({ locale: 'fr' });
	test('keeps the complete action labels reachable beside the workspace rail', async ({ page }) => {
		test.skip(
			!process.env.PP_E2E_WORKSPACE_FIXTURE,
			'Run against the synthetic workspace fixture.'
		);
		await page.setViewportSize({ width: 820, height: 820 });
		const mounted = page.waitForRequest((r) => new URL(r.url()).pathname === '/api/v1/events');
		await page.goto('/dashboard');
		await mounted;
		await page.getByRole('button', { name: 'Sélectionner Coast 02', exact: true }).press('Space');
		for (const width of [820, 1280]) {
			await page.setViewportSize({ width, height: 820 });
			const pin = page.getByRole('button', { name: 'Retirer des favoris' });
			await expect(pin).toBeInViewport({ ratio: 1 });
			expect(await pin.evaluate((el) => el.scrollWidth <= el.clientWidth + 1)).toBe(true);
			await expect(page.getByRole('button', { name: 'Supprimer', exact: true })).toBeInViewport({
				ratio: 1
			});
		}
	});
});

test('Studio places the favorite at the bottom right and keeps cover actions side by side', async ({
	page
}) => {
	test.skip(!process.env.PP_E2E_WORKSPACE_FIXTURE, 'Run against the synthetic workspace fixture.');
	await page.setViewportSize({ width: 1280, height: 800 });
	await page.goto('/dashboard/gallery-2');
	const cover = page.getByRole('button', { name: 'Unpin gallery' });
	const position = await cover.evaluate((button) => {
		const rect = button.getBoundingClientRect();
		const card = button.parentElement!.getBoundingClientRect();

		return { right: card.right - rect.right, bottom: card.bottom - rect.bottom };
	});
	expect(position.right).toBeLessThan(12);
	expect(position.bottom).toBeLessThan(12);

	const actions = page.locator('[data-slot="design-cover-actions"]');
	await expect(async () => {
		await page.getByRole('tab', { name: 'Design', exact: true }).click();
		await expect(actions).toBeVisible({ timeout: 750 });
	}).toPass({ timeout: 5000 });
	const change = (await actions.getByRole('button', { name: 'Change cover' }).boundingBox())!;
	const remove = (await actions.getByRole('button', { name: 'Remove cover' }).boundingBox())!;
	expect(change.y).toBe(remove.y);
	expect(change.x + change.width).toBeLessThan(remove.x);
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
