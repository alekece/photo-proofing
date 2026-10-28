import type { PageLoad } from './$types';

export const load: PageLoad = ({ params }) => ({ gallery: params.gallery });
