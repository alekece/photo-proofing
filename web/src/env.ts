import { defineEnvVars } from '@sveltejs/kit/env';
import { building, dev } from '$app/env';
import { resolveApiAddress } from '#lib/api-address.js';

export const variables = defineEnvVars({
	PP_API_INTERNAL_URL: {
		description: 'Address of the API as reachable from the web server, e.g. http://api:8080',
		schema: (value) => resolveApiAddress(value, { dev, building })
	}
});
