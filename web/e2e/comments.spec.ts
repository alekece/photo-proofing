import { expect, test } from '@playwright/test';

const token = process.env.PP_E2E_COMMENTS_TOKEN;

for (const viewport of [
	{ width: 1440, height: 900 },
	{ width: 390, height: 844 },
	{ width: 667, height: 375 }
]) {
	test(`proof comments fit beside or below the photo at ${viewport.width}×${viewport.height}`, async ({
		page
	}) => {
		test.skip(!token, 'Supply a disposable shared gallery containing a ready proof.');
		await page.setViewportSize(viewport);
		await page.goto(`/g/${encodeURIComponent(token!)}`);
		await page.getByRole('button', { name: /Open proof 1 of/ }).click();
		await page.getByRole('button', { name: 'Comments', exact: true }).click();

		const composer = page.getByRole('textbox');
		await expect(composer).toBeEnabled();
		const bounds = await page.locator('dialog').evaluate((dialog) => {
			const photo = dialog.querySelector('img')!.getBoundingClientRect();
			const panel = dialog.querySelector('aside')!.getBoundingClientRect();
			const form = dialog.querySelector('form')!.getBoundingClientRect();
			return {
				photoVisible: photo.width >= 100 && photo.height >= 80,
				photoSeparate: photo.right <= panel.left + 1 || photo.bottom <= panel.top + 1,
				composerFits: form.top >= panel.top && form.bottom <= panel.bottom,
				panelFits: panel.bottom <= window.innerHeight && panel.right <= window.innerWidth
			};
		});

		expect(bounds).toEqual({
			photoVisible: true,
			photoSeparate: true,
			composerFits: true,
			panelFits: true
		});
		await composer.fill('Can we try black and white?');
		await expect(page.getByRole('button', { name: 'Send', exact: true })).toBeEnabled();
		await page.keyboard.press('Escape');
		await expect(composer).toHaveCount(0);
		await expect(page.getByRole('dialog')).toBeVisible();
	});
}
