import type { StorybookConfig } from '@storybook/sveltekit';

export default {
	framework: '@storybook/sveltekit',
	core: { disableTelemetry: true },
	stories: ['../src/lib/components/ui/**/*.stories.svelte'],
	addons: ['@storybook/addon-svelte-csf', '@storybook/addon-mcp']
} satisfies StorybookConfig;
