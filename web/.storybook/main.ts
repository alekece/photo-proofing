import { fileURLToPath } from 'node:url';
import type { StorybookConfig } from '@storybook/sveltekit';

const photoFixture = fileURLToPath(
	new URL('../src/lib/components/ui/PhotoStack/storybookPhotos.ts', import.meta.url)
);

export default {
	framework: '@storybook/sveltekit',
	core: { disableTelemetry: true },
	features: { experimentalReview: false },
	stories: ['../src/lib/components/ui/**/*.stories.svelte'],
	addons: ['@storybook/addon-svelte-csf', '@storybook/addon-mcp'],
	viteFinal(config) {
		config.plugins ??= [];
		config.plugins.push({
			name: 'storybook-photo-stack-fixture',
			enforce: 'pre',
			resolveId(source, importer) {
				if (source === '#lib/gallery/photos.js' && importer?.endsWith('/PhotoStack.svelte')) {
					return photoFixture;
				}
			}
		});
		return config;
	}
} satisfies StorybookConfig;
