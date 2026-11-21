import '../src/app.css';
import { registerClientLocale } from '#lib/i18n/strategy.js';
import { baseLocale } from '#lib/paraglide/runtime.js';

document.documentElement.lang = baseLocale;
registerClientLocale();
