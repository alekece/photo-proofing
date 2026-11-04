import { redirect } from '@sveltejs/kit';
import type { PageServerLoad } from './$types';

/** The dashboard already sends signed-out visitors to sign-in or first-run setup. */
export const load: PageServerLoad = () => {
	redirect(303, '/dashboard');
};
