import { mkdir } from 'node:fs/promises';
import { join } from 'node:path';
import { expect, test } from '@playwright/test';

test.use({ viewport: { width: 1280, height: 800 }, reducedMotion: 'reduce' });

test('saves a real watermark and keeps Settings usable on desktop and phone', async ({ page }) => {
	test.setTimeout(60_000);
	await page.goto('/settings');
	await expect(page).toHaveURL(/\/login$/);
	const setup = await page
		.getByRole('heading', { name: 'Create your photographer account' })
		.isVisible();
	const password = 'synthetic settings trial password';
	await page.getByLabel('Email', { exact: true }).fill('trial@example.test');
	await page.getByLabel('Password', { exact: true }).fill(password);
	if (setup) await page.getByLabel('Confirm password', { exact: true }).fill(password);
	await page
		.getByRole('button', { name: setup ? 'Create account' : 'Sign in', exact: true })
		.click();
	await expect(page).toHaveURL(/\/dashboard$/, { timeout: 20_000 });
	await page.goto('/settings');
	await expect(page.getByRole('heading', { name: 'Settings', exact: true })).toBeVisible();
	await page.getByLabel('Studio name').fill('Studio Juniper');

	const png = await page.evaluate(() => {
		const canvas = document.createElement('canvas');
		canvas.width = 120;
		canvas.height = 60;
		const paint = canvas.getContext('2d')!;
		paint.fillStyle = '#111111';
		paint.beginPath();
		paint.arc(36, 30, 20, 0, Math.PI * 2);
		paint.fill();
		paint.fillStyle = '#ffffff';
		paint.fillRect(64, 10, 40, 40);
		return canvas.toDataURL('image/png').split(',')[1];
	});
	const remove = page.getByRole('button', { name: 'Remove logo' });
	if (await remove.count()) await remove.click();
	await page.locator('input[type=file]').setInputFiles({
		name: 'synthetic-logo.png',
		mimeType: 'image/png',
		buffer: Buffer.from(png, 'base64')
	});
	await expect(remove).toBeVisible();

	const watermark = page.getByRole('tab', { name: 'Watermark' });
	const preview = page.getByRole('img', { name: 'Preview of a proof with the watermark' });
	const both = page.getByRole('button', { name: 'Name and logo' });
	const hex = page.getByLabel('Hex color');
	const opacity = page.getByRole('slider', { name: 'Watermark opacity' });
	await watermark.click();
	await both.click();
	await hex.fill('#A12B3C');
	await opacity.fill('50');
	await page.getByRole('button', { name: 'Save changes' }).click();
	await expect(page.getByRole('button', { name: 'Saved', exact: true })).toBeVisible();
	await page.reload();
	await watermark.click();
	await expect(hex).toHaveValue('#A12B3C');
	await expect(opacity).toHaveValue('50');
	await expect(both).toHaveAttribute('aria-pressed', 'true');
	await expect(both).toHaveCSS('background-color', 'rgb(247, 227, 196)');
	const logo = both.locator('img');
	await expect(logo).toHaveCSS('filter', 'none');
	await expect(logo).toHaveCSS('box-shadow', 'none');
	const pixels = await logo.evaluate(async (logo: HTMLImageElement) => {
		await logo.decode();
		const canvas = document.createElement('canvas');
		canvas.width = logo.naturalWidth;
		canvas.height = logo.naturalHeight;
		const paint = canvas.getContext('2d')!;
		paint.drawImage(logo, 0, 0);
		return [36, 84, 0].map((x) => [...paint.getImageData(x, 30, 1, 1).data]);
	});
	expect(pixels).toEqual([
		[17, 17, 17, 255],
		[255, 255, 255, 255],
		[0, 0, 0, 0]
	]);
	expect(await page.evaluate(() => matchMedia('(prefers-reduced-motion: reduce)').matches)).toBe(
		true
	);

	const directory = process.env.PP_SETTINGS_CAPTURE_DIR;
	if (directory) await mkdir(directory, { recursive: true });
	const controls = page.locator('[data-settings-controls]');
	for (const [view, width, height] of [
		['desktop', 1280, 800],
		['phone', 390, 844]
	] as const) {
		await page.setViewportSize({ width, height });
		await expect(preview).toHaveCount(1);
		expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
			true
		);
		const previewBox = (await preview.boundingBox())!;
		const controlsBox = (await controls.boundingBox())!;
		if (view === 'desktop') expect(previewBox.x).toBeGreaterThan(controlsBox.x);
		else expect(previewBox.y).toBeGreaterThanOrEqual(controlsBox.y + controlsBox.height);
		await hex.scrollIntoViewIfNeeded();
		await expect(page.getByRole('button', { name: 'Save changes' })).toBeInViewport();
		await expect(page.getByRole('button', { name: 'Cancel' })).toBeInViewport();

		if (directory) {
			if (view === 'phone') {
				await both.evaluate((button) =>
					window.scrollBy(0, button.getBoundingClientRect().top - 84)
				);
			}
			await page.screenshot({ path: join(directory, `settings-watermark.${view}.png`) });
		}
	}

	await page.getByRole('button', { name: /language/i }).click();
	await page.getByRole('radio', { name: 'Français' }).click();
	await expect(page.getByRole('heading', { name: 'Réglages' })).toBeVisible();
	const french = page.getByRole('tablist', { name: 'Sections des réglages' });
	await french.getByRole('tab', { name: 'Studio' }).focus();
	await page.keyboard.press('End');
	const quality = french.getByRole('tab', { name: 'Qualité des photos' });
	await expect(quality).toBeFocused();
	await expect(quality).toHaveAttribute('aria-selected', 'true');
	const strip = await french.evaluate(({ scrollWidth, clientWidth }) => ({
		scrollWidth,
		clientWidth
	}));
	expect(strip.scrollWidth).toBeGreaterThan(strip.clientWidth);
	await expect.poll(() => french.evaluate((strip) => strip.scrollLeft)).toBeGreaterThan(0);
	expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
	await page.keyboard.press('Home');
	await page.keyboard.press('ArrowRight');
	await expect(french.getByRole('tab', { name: 'Filigrane' })).toBeFocused();
	await expect(page.getByLabel('Code hexadécimal')).toHaveValue('#A12B3C');
	await expect(page.getByRole('button', { name: 'Enregistrer' })).toBeInViewport();
	await expect(page.getByRole('button', { name: 'Annuler' })).toBeInViewport();
	await page.getByRole('button', { name: /langue/i }).click();
	await page.getByRole('radio', { name: 'English' }).click();
	await expect(page.getByRole('heading', { name: 'Settings' })).toBeVisible();
});
