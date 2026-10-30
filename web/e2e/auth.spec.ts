import { expect, test } from '@playwright/test';
import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);

test('real setup, persistent cookie, login, and revocable logout', async ({ browser, baseURL }) => {
	const context = await browser.newContext();
	const page = await context.newPage();
	await page.goto(`${baseURL}/dashboard`);
	await expect(page).toHaveURL(/\/login$/);
	const setup = await page
		.getByRole('heading', { name: 'Create your photographer account' })
		.isVisible();
	// The caller supplies a disposable installation; reruns can verify its persisted account.
	const password = 'synthetic trial password only';
	await page.getByLabel('Email', { exact: true }).fill('trial@example.test');
	await page.getByLabel('Password', { exact: true }).fill(password);
	await page
		.getByRole('button', {
			name: setup ? 'Create account' : 'Sign in',
			exact: true
		})
		.click();
	await expect(page.getByRole('heading', { name: 'Dashboard', exact: true })).toBeVisible();
	await expect(page.getByRole('checkbox')).toHaveCount(0);
	const storage = await context.storageState();
	const cookie = storage.cookies.find((c) => c.name === 'pp_session');
	expect(cookie?.httpOnly).toBe(true);
	expect(cookie?.expires).toBeGreaterThan(Date.now() / 1000);
	await context.close();
	const restored = await browser.newContext({ storageState: storage });
	const returned = await restored.newPage();
	await returned.goto(`${baseURL}/dashboard`);
	await expect(returned.getByRole('heading', { name: 'Dashboard', exact: true })).toBeVisible();
	const other = await browser.newContext();
	const otherPage = await other.newPage();
	await otherPage.goto(`${baseURL}/login`);
	await otherPage.getByLabel('Email', { exact: true }).fill('trial@example.test');
	await otherPage.getByLabel('Password', { exact: true }).fill(password);
	await otherPage.getByRole('button', { name: 'Sign in', exact: true }).click();
	await expect(otherPage.getByRole('heading', { name: 'Dashboard', exact: true })).toBeVisible();
	await returned.getByRole('button', { name: 'Sign out', exact: true }).click();
	await expect(returned).toHaveURL(/\/login$/);
	const replay = await browser.newContext({ storageState: storage });
	const response = await replay.request.post(`${baseURL}/api/v1/auth/renew`, {
		headers: { Origin: baseURL! }
	});
	expect(response.status()).toBe(401);
	await otherPage.reload();
	await expect(otherPage.getByRole('heading', { name: 'Dashboard', exact: true })).toBeVisible();
	await otherPage.setViewportSize({ width: 320, height: 800 });
	expect(
		await otherPage.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)
	).toBe(true);
	await otherPage.getByRole('button', { name: 'Sign out', exact: true }).click();
	await otherPage.getByLabel('Email', { exact: true }).fill('trial@example.test');
	await otherPage.getByLabel('Password', { exact: true }).fill('incorrect password');
	await otherPage.getByRole('button', { name: 'Sign in', exact: true }).click();
	await expect(otherPage.getByRole('alert')).toContainText('invalid email or password');
	await otherPage.addScriptTag({
		path: require.resolve('axe-core/axe.min.js')
	});
	const violations = await otherPage.evaluate(async () => {
		const axe = (
			window as unknown as {
				axe: { run: () => Promise<{ violations: unknown[] }> };
			}
		).axe;
		return (await axe.run()).violations;
	});
	expect(violations).toEqual([]);
	await otherPage.screenshot({ path: '/tmp/pp63-login-mobile.png' });
	await replay.close();
	await other.close();
	await restored.close();
});

test('activity extends a short session while idle browser traffic does not', async ({
	page,
	baseURL
}) => {
	const timeout = Number(process.env.PP_E2E_IDLE_MS ?? 0);
	test.setTimeout(60000);
	test.skip(!timeout, 'Run with an isolated API configured with a short idle timeout.');
	await page.goto('/login');
	await page.getByLabel('Email', { exact: true }).fill('trial@example.test');
	await page.getByLabel('Password', { exact: true }).fill('synthetic trial password only');
	await page.getByRole('button', { name: 'Sign in', exact: true }).click();
	await expect(page.getByRole('heading', { name: 'Dashboard', exact: true })).toBeVisible();
	await page.waitForTimeout(timeout * 0.6);
	const renewal = page.waitForResponse(
		(r) => r.url().endsWith('/auth/renew') && r.request().method() === 'POST'
	);
	await page.keyboard.press('Tab');
	expect((await renewal).status()).toBe(204);

	await page.waitForTimeout(timeout * 0.6);
	let status = await page.evaluate(() =>
		fetch('/api/v1/auth/status', { cache: 'no-store' }).then((r) => r.json())
	);

	expect(status.identity).not.toBeNull();
	await page.waitForTimeout(timeout + 500);
	status = await page.evaluate(() =>
		fetch('/api/v1/auth/status', { cache: 'no-store' }).then((r) => r.json())
	);
	expect(status.identity).toBeNull();
	await page.reload();
	await expect(page).toHaveURL(/\/login$/);
});
