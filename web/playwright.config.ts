import { defineConfig } from '@playwright/test';

export default defineConfig({
	testDir: './e2e',
	outputDir: process.env.PP_E2E_OUTPUT ?? '/tmp/pp63-playwright-results',
	workers: 1,
	fullyParallel: false,
	use: {
		baseURL: process.env.PP_E2E_URL ?? 'http://localhost:15163',
		headless: true,
		launchOptions: {
			executablePath: process.env.PP_BROWSER_PATH ?? '/usr/bin/google-chrome'
		}
	},
	reporter: 'list'
});
