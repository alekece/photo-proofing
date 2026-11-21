import { mkdir } from 'node:fs/promises';
import { join } from 'node:path';
import { expect, test } from '@playwright/test';

test.use({ viewport: { width: 1280, height: 800 }, reducedMotion: 'reduce' });

test('saves real watermark and signature settings and keeps the editor usable on desktop and phone', async ({
	page
}) => {
	test.setTimeout(60_000);
	await page.goto('/settings');
	await expect(page).toHaveURL(/\/login$/);
	const setup = await page
		.getByRole('heading', { name: 'Create your photographer account' })
		.isVisible();
	const password = 'synthetic trial password only';
	await page.getByLabel('Email', { exact: true }).fill('trial@example.test');
	await page.getByLabel('Password', { exact: true }).fill(password);
	if (setup) await page.getByLabel('Confirm password', { exact: true }).fill(password);
	await page
		.getByRole('button', { name: setup ? 'Create account' : 'Sign in', exact: true })
		.click();
	await expect(page).toHaveURL(/\/dashboard$/, { timeout: 20_000 });
	const mounted = page.waitForRequest(
		(request) => new URL(request.url()).pathname === '/api/v1/events'
	);
	await page.goto('/settings');
	await mounted;
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
	const signature = page.getByRole('tab', { name: 'Cover signature' });
	const preview = page.getByRole('img', { name: 'Preview of a proof with the watermark' });
	const both = page.getByRole('button', { name: 'Name and logo' });
	const hex = page.getByLabel('Hex color');
	const opacity = page.getByRole('slider', { name: 'Watermark opacity' });
	await signature.click();
	await page.getByRole('button', { name: 'Both', exact: true }).click();
	await page.getByLabel('Text color', { exact: true }).fill('#F6CCB4');
	await watermark.click();
	await both.click();
	await hex.fill('#A12B3C');
	await opacity.fill('50');
	await page.getByRole('button', { name: 'Save changes' }).click();
	await expect(page.getByRole('button', { name: 'Saved', exact: true })).toBeVisible();
	await page.reload();
	await expect(async () => {
		if ((await signature.getAttribute('aria-selected')) === 'false') await signature.click();
		await expect(signature).toHaveAttribute('aria-selected', 'true', { timeout: 500 });
	}).toPass({ timeout: 5000 });
	const printedName = page
		.getByRole('img', { name: 'Preview of a client cover with the signature' })
		.getByText('Studio Juniper', { exact: true });
	await expect(printedName).toHaveCSS('color', 'rgb(246, 204, 180)');
	await watermark.click();
	await expect(hex).toHaveValue('#A12B3C');
	await expect(opacity).toHaveValue('50');
	await expect(both).toHaveAttribute('aria-pressed', 'true');
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
		const hexBox = (await hex.boundingBox())!;
		expect(previewBox.y).toBeGreaterThanOrEqual(hexBox.y + hexBox.height);
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

	const settings = page.getByRole('main');
	await page.getByRole('tab', { name: 'General' }).click();
	await settings.getByRole('button', { name: /language/i }).click();
	await expect(page.getByRole('radio', { name: 'English' })).toBeInViewport({ ratio: 1 });
	await expect(page.getByRole('radio', { name: 'Français' })).toBeInViewport({ ratio: 1 });
	const frenchMounted = page.waitForRequest(
		(request) => new URL(request.url()).pathname === '/api/v1/events'
	);
	await page.getByRole('radio', { name: 'Français' }).click();
	await frenchMounted;
	await expect(page.getByRole('heading', { name: 'Réglages' })).toBeVisible();
	const french = page.getByRole('tablist', { name: 'Sections des réglages' });
	await french.getByRole('tab', { name: 'Général' }).focus();
	await page.keyboard.press('End');
	const frenchSignature = french.getByRole('tab', { name: 'Signature de couverture' });
	await expect(frenchSignature).toBeFocused();
	await expect(frenchSignature).toHaveAttribute('aria-selected', 'true');
	expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
	await page.keyboard.press('Home');
	await page.keyboard.press('ArrowRight');
	await expect(french.getByRole('tab', { name: 'Filigrane' })).toBeFocused();
	await expect(page.getByLabel('Code hexadécimal')).toHaveValue('#A12B3C');
	await expect(page.getByRole('button', { name: 'Enregistrer' })).toBeInViewport();
	await expect(page.getByRole('button', { name: 'Annuler' })).toBeInViewport();
	await french.getByRole('tab', { name: 'Général' }).click();
	await settings.getByRole('button', { name: /langue/i }).click();
	await page.getByRole('radio', { name: 'English' }).click();
	await expect(page.getByRole('heading', { name: 'Settings' })).toBeVisible();
});
