/** Shared by the Vite plugin and the `paraglide` compile script, so both emit the same runtime. */
export default {
	outdir: './src/lib/paraglide',
	strategy: ['custom-workspace', 'preferredLanguage', 'baseLocale']
};
