import { paraglideVitePlugin } from '@inlang/paraglide-js';
import tailwindcss from '@tailwindcss/vite';
import adapter from '@sveltejs/adapter-node';
import { sveltekit } from '@sveltejs/kit/vite';
import { svelteTesting } from '@testing-library/svelte/vite';
import { defineConfig } from 'vitest/config';

const buildSha = process.env.PP_BUILD_SHA ?? '';
if (buildSha && !/^[0-9a-f]{40}$/.test(buildSha)) {
	throw new Error('PP_BUILD_SHA must be a complete lowercase Git commit SHA.');
}

export default defineConfig({
	define: { 'import.meta.env.PP_BUILD_SHA': JSON.stringify(buildSha) },
	plugins: [
		// Options live in project.inlang/paraglide.config.js, shared with the `paraglide` script.
		paraglideVitePlugin({ project: './project.inlang' }),
		tailwindcss(),
		sveltekit({
			compilerOptions: {
				// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
				runes: ({ filename }) =>
					filename.split(/[/\\]/).includes('node_modules') ? undefined : true
			},

			adapter: adapter()
		}),
		svelteTesting()
	],
	server: {
		proxy: {
			'/api': process.env.PP_API_PROXY_TARGET ?? 'http://127.0.0.1:8080'
		}
	},
	test: {
		environment: 'jsdom',
		include: ['src/**/*.test.ts']
	}
});
