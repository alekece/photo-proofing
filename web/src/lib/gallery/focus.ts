/** Focus moving to Retry is not leaving for good: the user is about to retry the attempt. */
export const goingToRetry = (event: FocusEvent) =>
	event.relatedTarget instanceof HTMLElement && event.relatedTarget.dataset.retry !== undefined;
