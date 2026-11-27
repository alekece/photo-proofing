import { expect, test } from '@playwright/test';
import type { Locator } from '@playwright/test';

async function expectRoundHover(button: Locator, size: number, color: string) {
	await button.hover();
	await expect(button).toHaveCSS('cursor', 'pointer');
	await expect
		.poll(() =>
			button.evaluate((element) => {
				const disc = getComputedStyle(element, '::before');

				return [disc.width, disc.height, disc.borderRadius, disc.backgroundColor];
			})
		)
		.toEqual([`${size}px`, `${size}px`, '50%', color]);
}

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
	const search = page.getByRole('searchbox', {
		name: /^(Search galleries|Rechercher une galerie)$/
	});
	const status = page.getByRole('button', { name: /status|statut/i });
	const shared = page.getByRole('menuitemradio', { name: /shared|partagée/i });
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
		await expect(page).toHaveURL(/\/dashboard\/gallery-21$/);
		if (useBrowserBack) await page.goBack();
		else
			await page
				.getByRole('link', { name: /^(Galleries|Galeries)(?: \d+)?$/ })
				.first()
				.click();
		await expect(page).toHaveURL(/\/dashboard\?sort=title$/);
		await expect(search).toHaveValue('Coast');
		await expect(page.getByRole('link', { name: /coast 01|forest 28/i })).toHaveCount(0);
		await expect.poll(() => page.evaluate(() => window.scrollY)).toBe(position);
	}

	await gallery.click();
	await page.getByRole('button', { name: /Notifications, 1 (unread|non lue)/ }).click();
	await expect(
		page.getByRole('button', { name: /^(Mark all as read|Tout marquer comme lu)$/ })
	).toBeInViewport({
		ratio: 1
	});
	await page.getByRole('link', { name: /forest 28/i }).click();
	await expect(page).toHaveURL(/\/dashboard\/gallery-28$/);
	await page.goBack();
	await expect(page).toHaveURL(/\/dashboard\/gallery-21$/);
	await page
		.getByRole('link', { name: /^(Galleries|Galeries)(?: \d+)?$/ })
		.first()
		.click();
	await expect(search).toHaveValue('Coast');
	await expect.poll(() => page.evaluate(() => window.scrollY)).toBe(position);

	await page.getByRole('link', { name: /^(Activity|Activité)$/ }).click();
	await expect(page).toHaveURL(/\/activity$/);
	await page.getByRole('link', { name: 'Coast 01' }).first().click();
	await expect(page).toHaveURL(/\/dashboard\/gallery-1$/);
	await page.goBack();
	await expect(page).toHaveURL(/\/activity$/);
});

test('notification history stays reachable on short viewports after marking all read', async ({
	page
}) => {
	test.skip(!process.env.PP_E2E_WORKSPACE_FIXTURE, 'Run against the synthetic workspace fixture.');
	let readAt: string | null = null;
	await page.route('**/api/v1/notifications/read', (route) => {
		readAt = '2026-11-18T11:00:00Z';

		return route.fulfill({ status: 204 });
	});
	await page.route('**/api/v1/notifications', (route) =>
		route.fulfill({
			json: {
				unread: readAt ? 0 : 12,
				notifications: Array.from({ length: 12 }, (_, i) => ({
					id: `notification-${i}`,
					gallery_id: `gallery-${i + 1}`,
					title: `Gallery history ${i + 1}`,
					hero_id: null,
					kind: 'open',
					count: 1,
					selected: 0,
					updated_at: '2026-11-18T10:00:00Z',
					read_at: readAt
				}))
			}
		})
	);

	for (const width of [900, 390]) {
		readAt = null;
		await page.setViewportSize({ width, height: 390 });
		await page.goto('/dashboard');
		const bell = page.getByRole('button', { name: /Notifications, 12 (unread|non lues)/ });
		if (width === 900) {
			// The rail is collapsed at this width: the bell is an icon row with an unread dot.
			const settings = page.getByRole('link', { name: /^(Settings|Réglages|Paramètres)$/ });
			const bellBox = (await bell.boundingBox())!;
			const settingsBox = (await settings.boundingBox())!;
			expect(bellBox.x + bellBox.width / 2).toBeCloseTo(settingsBox.x + settingsBox.width / 2);
			const glyph = (await bell.locator('svg').boundingBox())!;
			const dot = (await bell.locator('span.size-2.bg-danger').boundingBox())!;
			expect(dot.y).toBeLessThan(glyph.y + glyph.height / 2);
			expect(dot.x).toBeGreaterThan(glyph.x + glyph.width / 2);
			await page.locator('[data-studio-palette]').evaluate((workspace) => {
				workspace.setAttribute('data-studio-palette', 'neutral');
			});
			await bell.hover();
			await expect(bell).toHaveCSS('cursor', 'pointer');
			await expect(bell).toHaveCSS('background-color', 'rgb(224, 222, 215)');
		} else {
			await expect(bell).toHaveCSS('width', '38px');
		}
		await bell.click();
		const last = page.getByRole('link', { name: /gallery history 12/i });
		const unreadActivity = /unread activity|activité non lue/i;
		await expect(last).toHaveAccessibleName(unreadActivity);
		await last.focus();
		await expect(last).toBeInViewport({ ratio: 1 });
		const markAll = page.getByRole('button', { name: /Mark all as read|Tout marquer comme lu/ });
		await expect(markAll).toBeInViewport({ ratio: 1 });
		await markAll.click();
		await expect(markAll).toBeDisabled();
		await expect(last).toBeVisible();
		await expect(last).not.toHaveAccessibleName(unreadActivity);
		const close = page.getByRole('button', { name: /^(Close|Fermer)$/ });
		await expect(close).toBeInViewport({ ratio: 1 });
		if (width === 900) {
			await expect(close).toHaveCSS('background-color', 'rgba(0, 0, 0, 0)');
			await expect(close).toHaveCSS('width', '44px');
			await expect(close).toHaveCSS('height', '44px');
			await page.locator('[data-studio-palette]').evaluate((workspace) => {
				workspace.setAttribute('data-studio-palette', 'warm');
			});
			await expectRoundHover(close, 32, 'rgb(241, 236, 230)');
		}
		await close.click({ position: { x: 4, y: 22 } });
		await expect(page.getByRole('button', { name: 'Notifications' })).toBeFocused();
	}
});

test('activity and contacts share the compact desktop overflow trigger', async ({ page }) => {
	test.skip(!process.env.PP_E2E_WORKSPACE_FIXTURE, 'Run against the synthetic workspace fixture.');
	await page.setViewportSize({ width: 1440, height: 900 });

	for (const [path, name] of [
		['/activity', /\d+ (more|autres?) photos?/],
		['/contacts', /\d+ (more galler|autres? galer)/]
	] as const) {
		await page.goto(path);
		const trigger = page.getByRole('button', { name }).first();
		await expect(trigger).toBeVisible();
		await page.locator('[data-studio-palette]').evaluate((workspace) => {
			workspace.setAttribute('data-studio-palette', 'neutral');
		});
		await expect(trigger).toHaveCSS('font-weight', '400');
		await expect(trigger).toHaveCSS('height', '44px');
		await expectRoundHover(trigger, 32, 'rgb(241, 241, 239)');
		await expect(async () => {
			if ((await trigger.getAttribute('aria-expanded')) === 'false') await trigger.press('Enter');
			await expect(trigger).toHaveAttribute('aria-expanded', 'true', { timeout: 500 });
		}).toPass({ timeout: 5000 });
	}
});

test('activity uses the available width for favorite thumbnails', async ({ page }) => {
	test.skip(!process.env.PP_E2E_WORKSPACE_FIXTURE, 'Run against the synthetic workspace fixture.');
	await page.setViewportSize({ width: 1200, height: 800 });
	await page.goto('/activity');
	const photos = page.getByRole('img', { name: 'Photo', exact: true });
	await expect(photos).toHaveCount(10);
	const rows = await photos.evaluateAll((images) =>
		images.map((image) => image.getBoundingClientRect().top)
	);
	expect(new Set(rows).size).toBe(1);
});

test('activity search finds older visitor names and stays scoped to the current gallery', async ({
	page
}) => {
	test.skip(!process.env.PP_E2E_WORKSPACE_FIXTURE, 'Run against the synthetic workspace fixture.');
	await page.setViewportSize({ width: 1440, height: 900 });
	const initialPage = page.waitForResponse((response) =>
		response.url().endsWith('/api/v1/timeline')
	);
	await page.goto('/activity');
	const initial = await (await initialPage).json();
	expect(initial.next).toBeTruthy();
	expect(initial.events).not.toContainEqual(expect.objectContaining({ gallery_id: 'gallery-21' }));
	await expect(page.getByRole('link', { name: 'Coast 01', exact: true }).first()).toBeVisible();
	const search = page.getByRole('searchbox', {
		name: /Search by gallery or visitor|Rechercher une galerie ou un visiteur/
	});
	await page.getByRole('button', { name: /^Date$/ }).click();
	await page.getByRole('menuitemradio', { name: /^(Last 7 days|7 derniers jours)$/ }).click();
	await page.getByRole('button', { name: /^Type$/ }).click();
	await page.getByRole('menuitemradio', { name: /^(Downloads|Téléchargements)$/ }).click();
	const firstMatch = page.waitForResponse((response) => {
		const url = new URL(response.url());

		return (
			url.pathname === '/api/v1/timeline' &&
			url.searchParams.get('q') === 'ELISE' &&
			url.searchParams.get('kind') === 'download' &&
			url.searchParams.get('range') === '7d' &&
			!url.searchParams.has('cursor')
		);
	});
	await search.fill('ELISE');
	expect((await firstMatch).ok()).toBeTruthy();
	await expect(page.getByRole('link', { name: 'Coast 21', exact: true })).toBeVisible();
	await expect(page.getByRole('link', { name: 'Coast 01', exact: true })).toHaveCount(0);
	await search.clear();
	await expect(
		page.getByRole('button', { name: /Type.*(Downloads|Téléchargements)/ })
	).toBeVisible();
	await expect(
		page.getByRole('button', { name: /Date.*(Last 7 days|7 derniers jours)/ })
	).toBeVisible();
	await page.getByRole('link', { name: 'Coast 21', exact: true }).click();
	await page.getByRole('tab', { name: /^(Activity|Activité)$/ }).click();
	const visitor = page.getByRole('searchbox', { name: /Search by visitor|Rechercher un visiteur/ });
	await expect(page.getByText('Élise Brume', { exact: true })).toBeVisible();
	await visitor.fill('Coast 21');
	await expect(page.getByText('Élise Brume', { exact: true })).toHaveCount(0);
	await visitor.fill('ELISE');
	await expect(page.getByText('Élise Brume', { exact: true })).toBeVisible();

	await page.setViewportSize({ width: 390, height: 800 });
	await page.goto('/activity');
	await expect(search).toBeInViewport();
	await expect(page.getByRole('button', { name: /^Date$/ })).toBeInViewport();
});

test('settings actions stay in place when tabs need different scrolling heights', async ({
	playwright,
	baseURL
}) => {
	test.skip(!process.env.PP_E2E_WORKSPACE_FIXTURE, 'Run against the synthetic workspace fixture.');
	const browser = await playwright.chromium.launch({
		executablePath: process.env.PP_BROWSER_PATH ?? '/usr/bin/google-chrome',
		ignoreDefaultArgs: ['--hide-scrollbars'],
		args: ['--disable-features=OverlayScrollbar']
	});
	try {
		const page = await browser.newPage({ baseURL, viewport: { width: 1440, height: 830 } });
		const hydrated = page.waitForRequest(
			(request) => new URL(request.url()).pathname === '/api/v1/events'
		);
		await page.goto('/settings');
		await hydrated;
		const save = page.getByRole('button', { name: /^(Save|Enregistrer)$/ });
		const cancel = page.getByRole('button', { name: /^(Cancel|Annuler)$/ });
		const initial = [await save.boundingBox(), await cancel.boundingBox()];
		expect(initial.every(Boolean)).toBe(true);

		const rail = page.getByRole('tablist');
		const railTop = (await rail.boundingBox())!.y;
		await page.evaluate(() => window.scrollTo(0, document.documentElement.scrollHeight));
		await expect.poll(() => page.evaluate(() => window.scrollY)).toBeGreaterThan(0);
		expect((await rail.boundingBox())!.y).toBe(railTop);
		await page.evaluate(() => window.scrollTo(0, 0));

		for (const name of [/^(Watermark|Filigrane)$/, /^(Cover signature|Signature de couverture)$/]) {
			await page.getByRole('tab', { name }).click();
			await expect
				.poll(async () => [await save.boundingBox(), await cancel.boundingBox()])
				.toEqual(initial);
		}
	} finally {
		await browser.close();
	}
});

test('finals show finished-photo download dates without owner download actions', async ({
	page
}) => {
	test.skip(!process.env.PP_E2E_WORKSPACE_FIXTURE, 'Run against the synthetic workspace fixture.');
	const hydrated = page.waitForRequest(
		(request) => new URL(request.url()).pathname === '/api/v1/events'
	);
	await page.goto('/dashboard/gallery-7');
	await hydrated;
	const finals = page.getByRole('tab', { name: /^(Retouched|Retouchées)\s+3$/ });
	await finals.click();
	const panel = page.getByRole('tabpanel');

	await expect(panel.getByRole('img')).toHaveCount(3);
	await expect(panel.getByText(/^(Downloaded|Téléchargée) /)).toHaveCount(2);
	await expect(panel.getByRole('button', { name: /^(Download|Télécharger)$/ })).toHaveCount(0);
});

test('Print favorites keep the date below the photo and preserve the heart icon', async ({
	page
}) => {
	test.skip(!process.env.PP_E2E_WORKSPACE_FIXTURE, 'Run against the synthetic workspace fixture.');
	const hydrated = page.waitForRequest(
		(request) => new URL(request.url()).pathname === '/api/v1/events'
	);
	await page.goto('/dashboard/gallery-2');
	await hydrated;
	await page.getByRole('tab', { name: /^(Favorites|Favoris)/ }).click();
	const caption = page.getByRole('tabpanel').locator('.tile-caption').first();
	const heart = caption.locator('svg');
	await expect(heart).toBeVisible();
	await page
		.locator('[data-studio-mode]')
		.evaluate((element) => element.setAttribute('data-studio-mode', 'print'));
	await expect(heart).toBeVisible();
	await expect(caption.locator('time')).toBeVisible();
});

test('Print gallery captions match the full-image type treatment', async ({ page }) => {
	test.skip(!process.env.PP_E2E_WORKSPACE_FIXTURE, 'Run against the synthetic workspace fixture.');
	await page.goto('/dashboard');
	await page
		.locator('[data-studio-mode]')
		.evaluate((element) => element.setAttribute('data-studio-mode', 'print'));
	const caption = page.locator('[data-slot="gallery-card-caption"]').first();
	const title = caption.getByRole('heading');
	const summary = caption.locator('p');

	await expect(caption).toHaveCSS('text-shadow', 'none');
	await expect(title).toHaveCSS('white-space', 'nowrap');
	await expect(title).toHaveCSS('text-overflow', 'ellipsis');
	await expect(summary).toHaveCSS('font-weight', '400');
	await expect(title).toHaveAttribute('title', /.+/);
	await expect(page.locator('[data-slot="gallery-card-surface"]').first()).toHaveCSS(
		'padding',
		'6px'
	);
	await expect(page.locator('[data-slot="gallery-card-surface"]').first()).toHaveCSS(
		'background-color',
		'rgb(251, 248, 243)'
	);
	await expect(caption).toHaveCSS('border-width', '0px');
	await expect(caption).toHaveCSS('box-shadow', 'none');
});

test('workspace Photos applies Print captions below each tile', async ({ page }) => {
	test.skip(!process.env.PP_E2E_WORKSPACE_FIXTURE, 'Run against the synthetic workspace fixture.');
	const hydrated = page.waitForRequest(
		(request) => new URL(request.url()).pathname === '/api/v1/events'
	);
	await page.goto('/photos');
	await hydrated;
	await page
		.locator('[data-studio-mode]')
		.evaluate((element) => element.setAttribute('data-studio-mode', 'print'));
	const tile = page
		.locator('li')
		.filter({ has: page.getByRole('img') })
		.first();
	const imageButton = tile.getByRole('button', { name: /^(Open|Ouvrir) / });
	const caption = tile.locator('.tile-caption');

	const imageBox = (await imageButton.boundingBox())!;
	const captionBox = (await caption.boundingBox())!;
	expect(captionBox.y).toBeGreaterThanOrEqual(imageBox.y + imageBox.height - 1);
	await expect(caption).toHaveCSS('position', 'static');
	await expect(caption).toHaveCSS('text-shadow', 'none');
	const frame = caption.locator('xpath=ancestor::*[@data-slot="photo-tile-frame"]');
	await expect(frame).toHaveCSS('padding', '6px');
	await expect(frame).toHaveCSS('background-color', 'rgb(251, 248, 243)');
});

test('photo captions reuse gallery typography and badge placement', async ({ page }) => {
	test.skip(!process.env.PP_E2E_WORKSPACE_FIXTURE, 'Run against the synthetic workspace fixture.');
	await page.goto('/dashboard');
	const galleryCaption = page.locator('[data-slot="gallery-card-caption"]').first();
	const gallerySummary = galleryCaption.locator('p');
	const galleryStyle = await gallerySummary.evaluate((element) => {
		const style = getComputedStyle(element);
		return { font: style.fontFamily, size: style.fontSize, weight: style.fontWeight };
	});

	await page.goto('/photos');
	await page
		.locator('[data-studio-mode]')
		.evaluate((element) => element.setAttribute('data-studio-mode', 'print'));
	const photoCaption = page.locator('.tile-caption').first();
	const photoTitleStyle = await photoCaption.evaluate((element) => {
		const style = getComputedStyle(element);
		return { font: style.fontFamily, size: style.fontSize, weight: style.fontWeight };
	});
	const badge = page.locator('[data-slot="photo-tile-badge"]').first();
	await expect(badge).toHaveAccessibleName(/^(Retouched photo|Photo retouchée)$/);
	const frame = badge.locator('xpath=ancestor::*[@data-slot="photo-tile-frame"]');
	const media = frame.locator('[data-slot="photo-tile-media"]');
	const badgeBox = (await badge.boundingBox())!;
	const mediaBox = (await media.boundingBox())!;
	const captionBox = (await frame.locator('.tile-caption').boundingBox())!;
	const badgeRadius = await badge.evaluate((element) => getComputedStyle(element).borderRadius);
	const galleryChipRadius = await page.goto('/dashboard').then(() =>
		page
			.locator('[data-slot="gallery-card-status"] [data-slot="status-chip"]')
			.first()
			.evaluate((element) => getComputedStyle(element).borderRadius)
	);

	expect(photoTitleStyle).toEqual(galleryStyle);
	expect(badgeRadius).toBe(galleryChipRadius);
	expect(Math.abs(badgeBox.x - (mediaBox.x + 8))).toBeLessThan(3);
	expect(Math.abs(mediaBox.y + mediaBox.height - 8 - (badgeBox.y + badgeBox.height))).toBeLessThan(
		3
	);
	expect(captionBox.y).toBeGreaterThanOrEqual(mediaBox.y + mediaBox.height - 1);
});

test('Print mode reserves the same bottom strip for photos without captions', async ({ page }) => {
	test.skip(!process.env.PP_E2E_WORKSPACE_FIXTURE, 'Run against the synthetic workspace fixture.');
	const hydrated = page.waitForRequest(
		(request) => new URL(request.url()).pathname === '/api/v1/events'
	);
	await page.goto('/dashboard/gallery-2');
	await hydrated;
	await page
		.locator('[data-studio-mode]')
		.evaluate((element) => element.setAttribute('data-studio-mode', 'print'));
	const frame = page.locator('[role="tabpanel"]:visible [data-slot="photo-tile-frame"]').first();
	await expect(frame.locator('.tile-caption')).toHaveCount(0);
	const image = frame.locator('[data-slot="photo-tile-media"]');
	const imageBox = (await image.boundingBox())!;
	const frameBox = (await frame.boundingBox())!;

	expect(frameBox.y + frameBox.height - (imageBox.y + imageBox.height)).toBeGreaterThanOrEqual(20);
});

test('gallery selection target stays clear of the mobile status row', async ({ page }) => {
	test.skip(!process.env.PP_E2E_WORKSPACE_FIXTURE, 'Run against the synthetic workspace fixture.');
	await page.setViewportSize({ width: 390, height: 844 });
	await page.goto('/dashboard');
	const card = page.getByRole('link', { name: /Coast 02/ }).locator('..');
	const select = card.getByRole('button', { name: /(?:Select|Sélectionner) Coast 02/ });
	const status = card.locator('[data-slot="gallery-card-status"]');

	const selectBox = (await select.boundingBox())!;
	const statusBox = (await status.boundingBox())!;
	const cardBox = (await card.boundingBox())!;
	const overlaps =
		selectBox.x < statusBox.x + statusBox.width &&
		selectBox.x + selectBox.width > statusBox.x &&
		selectBox.y < statusBox.y + statusBox.height &&
		selectBox.y + selectBox.height > statusBox.y;

	expect(overlaps).toBe(false);
	expect(statusBox.x - cardBox.x).toBeLessThanOrEqual(16);
});

test('gallery selection scales the whole card and its control together', async ({ page }) => {
	test.skip(!process.env.PP_E2E_WORKSPACE_FIXTURE, 'Run against the synthetic workspace fixture.');
	const hydrated = page.waitForRequest(
		(request) => new URL(request.url()).pathname === '/api/v1/events'
	);
	await page.goto('/dashboard');
	await hydrated;
	const item = page.locator('main ul li').first();
	const link = item.locator('[data-slot="gallery-card-surface"]');
	const select = item.getByRole('button', { name: /Select|Sélectionner/ });
	const selectBox = (await select.boundingBox())!;
	await select.click();
	await expect(item).toHaveCSS('scale', '0.975');
	const selectedBox = (await select.boundingBox())!;
	expect(selectedBox.width).toBeLessThan(selectBox.width);
	expect(selectedBox.x).not.toBe(selectBox.x);
	expect(await link.getAttribute('aria-pressed')).toBeNull();
	await select.click();
	await expect(page.locator('[data-slot="select-actions"]')).toHaveCount(0);
});

test('gallery collection tabs inherit the Print Polaroid frame', async ({ page }) => {
	test.skip(!process.env.PP_E2E_WORKSPACE_FIXTURE, 'Run against the synthetic workspace fixture.');
	const hydrated = page.waitForRequest(
		(request) => new URL(request.url()).pathname === '/api/v1/events'
	);
	await page.goto('/dashboard/gallery-2');
	await hydrated;
	await page
		.locator('[data-studio-mode]')
		.evaluate((element) => element.setAttribute('data-studio-mode', 'print'));

	for (const tab of [/^Photos/, /^(Favorites|Favoris)/, /^(Retouched|Retouchées)/]) {
		await page.getByRole('tab', { name: tab }).click();
		const frame = page.locator('[role="tabpanel"]:visible [data-slot="photo-tile-frame"]').first();
		await expect(frame).toHaveCSS('padding', '6px');
		await expect(frame).toHaveCSS('background-color', 'rgb(251, 248, 243)');
	}
});

test('selection shrinks the complete Print frame and stays on the left', async ({ page }) => {
	test.skip(!process.env.PP_E2E_WORKSPACE_FIXTURE, 'Run against the synthetic workspace fixture.');
	const hydrated = page.waitForRequest(
		(request) => new URL(request.url()).pathname === '/api/v1/events'
	);
	await page.goto('/photos');
	await hydrated;
	const item = page.locator('main ul li').first();
	const tile = item.locator('[data-slot="photo-tile-frame"]');
	const select = item.getByRole('button', { name: /Select|Sélectionner/ });
	const selectBox = (await select.boundingBox())!;
	const tileBox = (await tile.boundingBox())!;

	expect(selectBox.x).toBeLessThan(tileBox.x + tileBox.width / 2);
	await select.click();
	await expect(tile).toHaveCSS('scale', '0.975');
});

test('gallery Design preview is inert and compact choices stay on one row', async ({ page }) => {
	test.skip(!process.env.PP_E2E_WORKSPACE_FIXTURE, 'Run against the synthetic workspace fixture.');
	const hydrated = page.waitForRequest(
		(request) => new URL(request.url()).pathname === '/api/v1/events'
	);
	await page.goto('/dashboard/gallery-2');
	await hydrated;
	await page.getByRole('tab', { name: 'Design', exact: true }).click();
	const preview = page.getByRole('region', { name: /^(Client preview|Aperçu client)$/ });

	await expect(preview.locator('[data-heading-font]')).toHaveAttribute('inert', '');
	const previewButtons = preview.locator('[data-heading-font] button');
	expect(await previewButtons.count()).toBeGreaterThan(0);
	expect(
		await previewButtons.first().evaluate((button) => {
			button.focus();
			return document.activeElement === button;
		})
	).toBe(false);
	const box = (await previewButtons.first().boundingBox())!;
	await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
	await expect(page.getByRole('dialog')).toHaveCount(0);
	await page.setViewportSize({ width: 390, height: 844 });
	const density = page.getByRole('group', { name: /^(Grid density|Densité de la grille)$/ });
	const label = density.locator('xpath=..').locator(':scope > span').first();
	const labelBox = (await label.boundingBox())!;
	const choicesBox = (await density.boundingBox())!;
	expect(
		Math.abs(labelBox.y + labelBox.height / 2 - (choicesBox.y + choicesBox.height / 2))
	).toBeLessThan(8);
	await expect(page.getByRole('heading', { name: 'Style' })).toBeVisible();
	await page.getByRole('button', { name: /^(Open full preview|Ouvrir l’aperçu complet)$/ }).click();
	const fullPreview = page.getByRole('dialog', { name: /^(Client preview|Aperçu client)$/ });
	await expect(fullPreview.locator('[data-heading-font]')).toHaveAttribute('inert', '');
	const close = fullPreview.getByRole('button', { name: /^(Close preview|Fermer l’aperçu)$/ });
	const desktop = fullPreview.getByRole('button', { name: /^(Desktop|Ordinateur)$/ });
	const phone = fullPreview.getByRole('button', { name: /^(Phone|Téléphone)$/ });
	await close.focus();
	await page.keyboard.press('Tab');
	await expect(desktop).toBeFocused();
	await page.keyboard.press('Tab');
	await expect(phone).toBeFocused();
	await page.keyboard.press('Tab');
	await expect(close).toBeFocused();
});

test('gallery comments identify personal-link clients and keep anonymous clients generic', async ({
	page
}) => {
	test.skip(!process.env.PP_E2E_WORKSPACE_FIXTURE, 'Run against the synthetic workspace fixture.');
	const hydrated = page.waitForRequest(
		(request) => new URL(request.url()).pathname === '/api/v1/events'
	);
	await page.goto('/dashboard/gallery-2');
	await hydrated;
	await page.getByRole('tab', { name: /^(Comments|Commentaires)\s+1$/ }).click();
	const panel = page.getByRole('tabpanel');
	await expect(panel.getByRole('searchbox')).toHaveCount(0);
	await panel.getByRole('button', { name: /Coast-2\.jpg/ }).click();
	await expect(panel.getByText('Camille Lune', { exact: true })).toHaveCount(2);
	await expect(panel.getByText(/^(Client|Client anonyme)$/)).toHaveCount(1);
});

test('gallery Contacts uses the full directory with reversible associations', async ({ page }) => {
	test.skip(!process.env.PP_E2E_WORKSPACE_FIXTURE, 'Run against the synthetic workspace fixture.');
	const hydrated = page.waitForRequest(
		(request) => new URL(request.url()).pathname === '/api/v1/events'
	);
	await page.goto('/dashboard/gallery-2');
	await hydrated;
	await page.getByRole('tab', { name: /^Contacts(?:\s+\d+)?$/ }).click();
	const panel = page.getByRole('tabpanel');

	await expect(panel.getByRole('listitem')).toHaveCount(1);
	await panel.getByRole('button', { name: /^(Manage contacts|Gérer les contacts)$/ }).click();
	const picker = page.getByRole('dialog', { name: /^(Link a contact|Associer un contact)$/ });
	const rows = picker.getByRole('listitem');
	const search = picker.getByRole('searchbox', { name: /Search by name|Rechercher par nom/ });
	const elise = picker.getByRole('switch', { name: /^(Link|Associer) Élise Brume$/ });
	await expect(rows).toHaveCount(6);
	await search.fill('Élise');
	await expect(rows).toHaveCount(1);
	await expect(elise).not.toBeChecked();

	await elise.click();
	await expect(elise).toBeChecked();
	await picker.getByRole('button', { name: /^(Cancel|Annuler)$/ }).click();
	await expect(picker).toBeHidden();
	await expect(panel.getByRole('listitem')).toHaveCount(1);

	await panel.getByRole('button', { name: /^(Manage contacts|Gérer les contacts)$/ }).click();
	await expect(elise).not.toBeChecked();
	await search.fill('Élise');
	await elise.click();
	await picker.getByRole('button', { name: /^(Done|Terminé)$/ }).click();
	await expect(picker).toBeHidden();
	await expect(panel.getByRole('listitem')).toHaveCount(2);

	await panel.getByRole('button', { name: /^(Manage contacts|Gérer les contacts)$/ }).click();
	await search.fill('Élise');
	await expect(elise).toBeChecked();
	await elise.click();
	await picker.getByRole('button', { name: /^(Done|Terminé)$/ }).click();
	await expect(picker).toBeHidden();
	await expect(panel.getByRole('listitem')).toHaveCount(1);
	await page.getByRole('button', { name: /^(New contact|Nouveau contact)$/ }).click();
	const dialog = page.getByRole('dialog', { name: /^(New contact|Nouveau contact)$/ });
	await expect(dialog.getByRole('textbox', { name: /^(Name|Nom)\b/ })).toBeVisible();
	await dialog.getByRole('button', { name: /^(Cancel|Annuler)$/ }).click();
});

test('the smallest repeated watermark reaches both sides and both ends of the preview', async ({
	page
}) => {
	test.skip(!process.env.PP_E2E_WORKSPACE_FIXTURE, 'Run against the synthetic workspace fixture.');
	await page.setViewportSize({ width: 1440, height: 900 });
	const hydrated = page.waitForRequest(
		(request) => new URL(request.url()).pathname === '/api/v1/events'
	);
	await page.goto('/settings');
	await hydrated;
	await page.getByRole('textbox', { name: /^(Studio name|Nom du studio)$/ }).fill('X');
	await page.getByRole('tab', { name: /^(Watermark|Filigrane)$/ }).click();
	const mark = page.getByRole('button', { name: /^(Name|Nom)$/ });
	const buttonBox = (await mark.boundingBox())!;
	const glyphBox = (await mark.getByText('Aa', { exact: true }).boundingBox())!;
	expect(glyphBox.y + glyphBox.height / 2).toBeCloseTo(buttonBox.y + buttonBox.height / 2, 0);

	await page.getByRole('button', { name: /^(Repeat|Répété)$/ }).click();
	await page.getByRole('slider', { name: /^(Size|Taille)$/ }).press('Home');
	const preview = page.getByRole('img', { name: /Preview of a proof|Aperçu d.une épreuve/ });
	await expect
		.poll(async () => {
			const image = (await preview.boundingBox())!;
			const zones = await preview.getByText('X', { exact: true }).evaluateAll((marks, frame) => {
				const seen = new Set<string>();

				for (const mark of marks) {
					if (getComputedStyle(mark).visibility === 'hidden') continue;

					const box = mark.getBoundingClientRect();
					const x = (box.x + box.width / 2 - frame.x) / frame.width;
					const y = (box.y + box.height / 2 - frame.y) / frame.height;

					if (x <= 0 || x >= 1 || y <= 0 || y >= 1) continue;
					if (x < 0.2) seen.add('left');
					if (x > 0.8) seen.add('right');
					if (y < 0.2) seen.add('top');
					if (y > 0.8) seen.add('bottom');
				}

				return [...seen].sort();
			}, image);

			return zones;
		})
		.toEqual(['bottom', 'left', 'right', 'top']);
	await expect(page.getByText(/^(Enabled per gallery|Activation par galerie)$/)).toHaveCount(0);
});

test('workspace and Settings navigation share transparent resting and distinct hover states', async ({
	page
}) => {
	test.skip(!process.env.PP_E2E_WORKSPACE_FIXTURE, 'Run against the synthetic workspace fixture.');
	await page.setViewportSize({ width: 1200, height: 800 });
	const hydrated = page.waitForRequest(
		(request) => new URL(request.url()).pathname === '/api/v1/events'
	);
	await page.goto('/settings');
	await hydrated;
	const rail = page.getByRole('banner');
	const contacts = rail.getByRole('link', { name: 'Contacts' });
	const settings = rail.getByRole('link', { name: /^(Settings|Réglages)$/ });
	const general = page.getByRole('tab', { name: /^(General|Général)$/ });
	const watermark = page.getByRole('tab', { name: /^(Watermark|Filigrane)$/ });
	const chosen = page
		.locator('label.control-choice')
		.filter({
			has: page.getByRole('radio', { checked: true })
		})
		.first();
	for (const [palette, railHover, tabHover, selected] of [
		['neutral', 'rgb(224, 222, 215)', 'rgb(241, 241, 239)', 'rgb(38, 37, 31)'],
		['warm', 'rgb(224, 214, 200)', 'rgb(241, 236, 230)', 'rgb(43, 36, 32)']
	]) {
		await page.locator('[data-studio-palette]').evaluate((workspace, value) => {
			workspace.setAttribute('data-studio-palette', value);
		}, palette);
		await page.mouse.move(600, 0);
		for (const [item, hover] of [
			[contacts, railHover],
			[watermark, tabHover]
		] as const) {
			await expect(item).toHaveCSS('background-color', 'rgba(0, 0, 0, 0)');
			await item.hover();
			await expect(item).toHaveCSS('background-color', hover);
		}
		for (const item of [settings, general, chosen]) {
			await item.hover();
			await expect(item).toHaveCSS('background-color', selected);
		}
	}
	await expect(rail.getByRole('button', { name: /^Language:/ })).toHaveCount(0);
	await expect(
		page.getByRole('radio', { name: /^(English|Français)$/, checked: true })
	).toBeVisible();
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
			const language = page.getByRole('radio', { name });
			// The phone layout stacks the Settings sections; the languages sit below the first screen.
			if (width < 640) await language.scrollIntoViewIfNeeded();
			await expect(language).toBeInViewport({ ratio: 1 });
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

test('a keyboard language choice stays in the draft after a failed save', async ({ page }) => {
	test.skip(!process.env.PP_E2E_WORKSPACE_FIXTURE, 'Run against the synthetic workspace fixture.');
	let requests = 0;
	await page.route('**/api/v1/settings', (route) => {
		if (route.request().method() !== 'PUT') return route.continue();
		requests++;
		return route.fulfill({ status: 500 });
	});
	const hydrated = page.waitForRequest(
		(request) => new URL(request.url()).pathname === '/api/v1/events'
	);
	await page.goto('/settings');
	await hydrated;
	const english = page.getByRole('radio', { name: 'English' });
	const french = page.getByRole('radio', { name: 'Français' });
	const save = page.getByRole('button', { name: 'Save', exact: true });
	await english.press('ArrowRight');
	await expect(french).toBeChecked();
	expect(requests).toBe(0);
	await save.click();
	await expect(page.getByText('Unable to save the settings.')).toBeVisible();
	await expect(french).toBeChecked();
	await expect(save).toBeEnabled();
	expect(requests).toBe(1);
});

test('the locked access-code helper sits below its label and Copy keeps its width', async ({
	page
}) => {
	test.skip(!process.env.PP_E2E_WORKSPACE_FIXTURE, 'Run against the synthetic workspace fixture.');
	await page.setViewportSize({ width: 390, height: 844 });
	await page.goto('/dashboard/gallery-2');
	await page.context().grantPermissions(['clipboard-read', 'clipboard-write']);
	const toggle = page.getByRole('switch', {
		name: /^(Require access code|Exiger un code d’accès)$/
	});
	await expect(async () => {
		await page.getByRole('tab', { name: /^(Settings|Réglages)$/ }).click();
		await expect(toggle).toBeVisible({ timeout: 500 });
	}).toPass({ timeout: 5000 });
	await expect(toggle).toBeDisabled();
	const reason = page.getByText(
		/^(Cannot add a code after sharing\.|Impossible d’ajouter un code après le partage\.)$/
	);
	await expect(toggle).toHaveAccessibleDescription(
		/Cannot add a code after sharing\.|Impossible d’ajouter un code après le partage\./
	);
	const label = (await page
		.getByText(/^(Require access code|Exiger un code d’accès)$/)
		.boundingBox())!;
	const reasonBox = (await reason.boundingBox())!;
	const control = (await toggle.boundingBox())!;
	expect(reasonBox.y).toBeGreaterThanOrEqual(label.y + label.height - 1);
	expect(control.x).toBeGreaterThan(label.x);
	const copy = page.getByRole('button', { name: /^(Copy|Copier)$/ });
	const before = (await copy.boundingBox())!.width;
	await copy.click();
	const copied = page.getByRole('button', { name: /^(Copied|Copié)$/ });
	await expect(copied).toBeVisible();
	expect((await copied.boundingBox())!.width).toBe(before);
	const check = (await copied.locator('svg').boundingBox())!;
	const face = (await copied.boundingBox())!;
	expect(Math.abs(check.x + check.width / 2 - (face.x + face.width / 2))).toBeLessThan(1);
	expect(Math.abs(check.y + check.height / 2 - (face.y + face.height / 2))).toBeLessThan(1);
	await expect(page.getByRole('status').filter({ hasText: /Copied|Copié/ })).toHaveCount(1);
});

test('photo filters are directly reachable and fit the phone toolbar', async ({ page }) => {
	test.skip(!process.env.PP_E2E_WORKSPACE_FIXTURE, 'Run against the synthetic workspace fixture.');
	await page.setViewportSize({ width: 390, height: 844 });
	const hydrated = page.waitForRequest(
		(request) => new URL(request.url()).pathname === '/api/v1/events'
	);
	await page.goto('/photos');
	await hydrated;
	const search = page.getByRole('searchbox', { name: 'Search by gallery' });
	const type = page.getByRole('button', { name: /^Type:/ });
	const sort = page.getByRole('button', { name: /^Sort:/ });
	await expect(search).toBeInViewport({ ratio: 1 });
	await expect(type).toBeInViewport({ ratio: 1 });
	await expect(sort).toBeInViewport({ ratio: 1 });
	const typeBox = (await type.boundingBox())!;
	const sortBox = (await sort.boundingBox())!;
	expect(typeBox.y).toBe(sortBox.y);
	expect(typeBox.x + typeBox.width).toBeLessThan(sortBox.x);
	await expect(page.getByRole('button', { name: /^Gallery:/ })).toHaveCount(0);
	const visible = page.getByRole('list', { name: 'Photos' }).getByRole('listitem');
	await search.fill('Coast 02');
	await expect(visible).toHaveCount(3);
	await type.click();
	await page.getByRole('menuitemradio', { name: 'Retouched', exact: true }).click();
	await expect(page.getByText('No photos match these filters.')).toBeVisible();
	await type.click();
	await page.getByRole('menuitemradio', { name: 'Unretouched' }).click();
	await expect(visible).toHaveCount(3);
	await expect(visible.first()).toContainText('Coast 02');
});

test('a long gallery title leaves status and selection visible on a phone', async ({ page }) => {
	test.skip(!process.env.PP_E2E_WORKSPACE_FIXTURE, 'Run against the synthetic workspace fixture.');
	await page.setViewportSize({ width: 390, height: 844 });
	await page.goto('/dashboard');
	const card = page.locator('a[href="/dashboard/gallery-3"]');
	const chip = card.locator('[data-slot="gallery-card-status"]');
	const bounds = (await card.boundingBox())!;
	const status = (await chip.boundingBox())!;
	expect(status.y).toBeGreaterThanOrEqual(bounds.y);
	const select = card.locator('..').getByRole('button');
	const target = (await select.boundingBox())!;
	const overlaps =
		target.x < status.x + status.width &&
		target.x + target.width > status.x &&
		target.y < status.y + status.height &&
		target.y + target.height > status.y;

	expect(overlaps).toBe(false);
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

test.describe('short touch viewport', () => {
	test.use({ hasTouch: true, viewport: { width: 390, height: 480 } });
	test('an upward-opening toolbar menu accepts taps over the workspace header', async ({
		page
	}) => {
		test.skip(
			!process.env.PP_E2E_WORKSPACE_FIXTURE,
			'Run against the synthetic workspace fixture.'
		);
		const mounted = page.waitForRequest((r) => new URL(r.url()).pathname === '/api/v1/events');
		await page.goto('/photos');
		await mounted;
		const trigger = page.getByRole('button', { name: /^Type:/ });
		await trigger.tap();
		const all = page.getByRole('menuitemradio', { name: 'All types' });
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
		const iconOffset = (button: Locator) =>
			button.evaluate((element) => {
				const bounds = element.getBoundingClientRect();
				const icon = element
					.querySelector('span[aria-hidden="false"] svg')!
					.getBoundingClientRect();

				return (icon.left + icon.right - bounds.left - bounds.right) / 2;
			});
		// Below lg the bar shows icons only; from lg the complete labels sit beside them.
		for (const width of [820, 1280]) {
			await page.setViewportSize({ width, height: 820 });
			const pin = page.getByRole('button', { name: 'Retirer des favoris' });
			await expect(pin).toBeInViewport({ ratio: 1 });
			expect(await pin.evaluate((el) => el.scrollWidth <= el.clientWidth + 1)).toBe(true);
			if (width < 1024) {
				expect(Math.abs(await iconOffset(pin))).toBeLessThan(1);
			} else {
				const { left, right } = await pin.evaluate((button) => {
					const bounds = button.getBoundingClientRect();
					const face = button.querySelector('span[aria-hidden="false"]')!;
					const icon = face.querySelector('svg')!.getBoundingClientRect();
					const label = Array.from(face.children).find(
						(child) => child.textContent === 'Retirer des favoris'
					)!;
					const text = document.createRange();
					text.selectNodeContents(label);

					return {
						left: icon.left - bounds.left,
						right: bounds.right - text.getBoundingClientRect().right
					};
				});
				expect(Math.abs(left - right)).toBeLessThan(1);
			}
			await expect(page.getByRole('button', { name: 'Supprimer', exact: true })).toBeInViewport({
				ratio: 1
			});
		}
		await page.setViewportSize({ width: 390, height: 820 });
		const pin = page.getByRole('button', { name: 'Retirer des favoris' });
		await expect(pin).toBeInViewport({ ratio: 1 });
		expect(Math.abs(await iconOffset(pin))).toBeLessThan(1);
	});
});

test('Studio groups the gold favorite at bottom-left and balances cover actions', async ({
	page
}) => {
	test.skip(!process.env.PP_E2E_WORKSPACE_FIXTURE, 'Run against the synthetic workspace fixture.');
	await page.setViewportSize({ width: 1280, height: 800 });
	await page.goto('/dashboard/gallery-2');
	const cover = page.getByRole('button', {
		name: /^(Unpin gallery|Retirer l’épingle de la galerie)$/
	});
	const controls = page.locator('[data-slot="gallery-cover-controls"]');
	const position = await controls.evaluate((element) => {
		const button = element.querySelector('button')!;
		const status = element.querySelector('[data-slot="status-chip"]')!;
		const rect = element.getBoundingClientRect();
		const card = element.parentElement!.getBoundingClientRect();

		return {
			left: rect.left - card.left,
			bottom: card.bottom - rect.bottom,
			gap: button.getBoundingClientRect().left - status.getBoundingClientRect().right
		};
	});
	expect(position.left).toBeLessThan(12);
	expect(position.bottom).toBeLessThan(12);
	expect(position.gap).toBeGreaterThanOrEqual(4);
	await expect(cover).toHaveCSS('color', 'rgb(200, 150, 30)');

	const actions = page.locator('[data-slot="design-cover-actions"]');
	await expect(async () => {
		await page.getByRole('tab', { name: 'Design', exact: true }).click();
		await expect(actions).toBeVisible({ timeout: 750 });
	}).toPass({ timeout: 5000 });
	const change = (await actions.getByRole('button', { name: /^(Change|Changer)$/ }).boundingBox())!;
	const remove = (await actions.getByRole('button', { name: /^(Remove|Retirer)$/ }).boundingBox())!;
	expect(change.y).toBe(remove.y);
	expect(change.x + change.width).toBeLessThan(remove.x);
	expect(Math.abs(change.width - remove.width)).toBeLessThan(1);
});

test('the gallery form closes back to its trigger, including with reduced motion', async ({
	page
}) => {
	test.skip(!process.env.PP_E2E_WORKSPACE_FIXTURE, 'Run against the synthetic workspace fixture.');
	await page.goto('/dashboard');
	const trigger = page.getByRole('button', { name: 'New gallery', exact: true });
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
