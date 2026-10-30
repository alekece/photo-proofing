import { goto } from '$app/navigation';
import { authRequest, createActivityRenewal } from '#lib/auth.js';

/** Browser-driven session renewal shared by the signed-in pages; expiry sends the user to sign-in. */
export function createSessionActivity(idleTimeoutMs: number) {
	return createActivityRenewal(
		idleTimeoutMs,
		() => authRequest('renew'),
		() => {
			void goto('/login?expired=1', { refreshAll: true });
		}
	);
}

/** Counts every pointer or key interaction as activity; returns the cleanup function. */
export function listenForActivity(activity: () => Promise<void>): () => void {
	const renew = () => {
		void activity();
	};
	window.addEventListener('pointerdown', renew);
	window.addEventListener('keydown', renew);
	return () => {
		window.removeEventListener('pointerdown', renew);
		window.removeEventListener('keydown', renew);
	};
}
