const STORAGE_KEY = 'pp-rail-workspace';
/** Below this width the workspace rail starts collapsed. */
const NARROW = '(max-width: 63.99rem)';

type Choice = 'collapsed' | 'expanded';

/**
 * Whether the workspace rail is collapsed to its icons. It follows the screen width until the
 * photographer picks a state; that choice is kept in the browser and mirrored on `<html>`, where
 * the stylesheet resolves the rail width.
 */
class RailState {
	#choice = $state<Choice | undefined>();
	#narrow = $state(false);

	get collapsed(): boolean {
		return (this.#choice ?? (this.#narrow ? 'collapsed' : 'expanded')) === 'collapsed';
	}

	toggle() {
		const next = this.collapsed ? 'expanded' : 'collapsed';

		this.#choice = next;
		this.#mirror(next);

		try {
			localStorage.setItem(STORAGE_KEY, next);
		} catch {
			// A browser that refuses storage still collapses for this visit.
		}
	}

	/** Restores the saved choice and follows the screen width; returns what stops that. */
	start(): () => void {
		this.#choice = this.#read();
		this.#mirror(this.#choice);

		const query = window.matchMedia?.(NARROW);

		if (!query) return () => {};

		const follow = () => (this.#narrow = query.matches);

		follow();
		query.addEventListener('change', follow);

		return () => query.removeEventListener('change', follow);
	}

	#read(): Choice | undefined {
		try {
			const saved = localStorage.getItem(STORAGE_KEY);

			return saved === 'collapsed' || saved === 'expanded' ? saved : undefined;
		} catch {
			return undefined;
		}
	}

	#mirror(choice: Choice | undefined) {
		if (choice) document.documentElement.dataset.workspaceRail = choice;
		else delete document.documentElement.dataset.workspaceRail;
	}
}

export const rails = new RailState();
