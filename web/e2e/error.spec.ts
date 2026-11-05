import { expect, test } from '@playwright/test';

test('an unknown address shows the app error page with focus on its heading', async ({
	page,
	baseURL
}) => {
	const response = await page.goto(`${baseURL}/no-such-page`);

	expect(response?.status()).toBe(404);

	const heading = page.getByRole('heading', { level: 1, name: "We couldn't find this page." });

	await expect(heading).toBeFocused();
	await expect(page.getByRole('link', { name: 'Go to sign in' })).toHaveAttribute('href', '/login');
});
