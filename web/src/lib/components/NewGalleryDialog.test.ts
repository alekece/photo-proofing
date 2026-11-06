import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('$app/navigation', () => ({ goto: vi.fn() }));
import { goto } from '$app/navigation';
import { GalleryApiError, type Gallery } from '#lib/gallery/api.js';
import NewGalleryDialog from './NewGalleryDialog.svelte';

const created = { id: 'new-id', title: null } as Gallery;

function setup() {
	const create = vi.fn().mockResolvedValue(created);
	const update = vi.fn().mockResolvedValue(created);
	const onclose = vi.fn();
	const user = userEvent.setup();
	const view = render(NewGalleryDialog, { onclose, create, update });

	return { create, update, onclose, user, ...view };
}

const titleField = () => screen.getByLabelText('Title') as HTMLInputElement;
const submit = (user: ReturnType<typeof userEvent.setup>) =>
	user.click(screen.getByRole('button', { name: 'Create' }));

beforeEach(() => vi.mocked(goto).mockClear());

describe('NewGalleryDialog', () => {
	it('creates the gallery, saves the title and opens it', async () => {
		const { create, update, user } = setup();

		await user.type(titleField(), '  Martin wedding ');
		await submit(user);

		await waitFor(() => expect(goto).toHaveBeenCalledWith('/dashboard/new-id'));
		expect(create).toHaveBeenCalledOnce();
		expect(update).toHaveBeenCalledWith('new-id', { title: 'Martin wedding' });
	});

	it('rejects a whitespace-only title', async () => {
		const { create, user } = setup();

		await user.type(titleField(), '   ');
		await submit(user);

		expect(screen.getByRole('alert').textContent).toMatch(/title/i);
		expect(create).not.toHaveBeenCalled();
	});

	it('rejects a 256-character title and accepts 255', async () => {
		const { create, user } = setup();

		await user.click(titleField());
		await user.paste('x'.repeat(256));
		await submit(user);

		expect(screen.getByRole('alert').textContent).toMatch(/255/);
		expect(create).not.toHaveBeenCalled();

		await user.clear(titleField());
		await user.paste('x'.repeat(255));
		await submit(user);

		await waitFor(() => expect(create).toHaveBeenCalledOnce());
	});

	it('still opens the created gallery when saving the title fails', async () => {
		const { update, user } = setup();
		update.mockRejectedValue(new GalleryApiError(500, 'boom'));

		await user.type(titleField(), 'Portraits');
		await submit(user);

		await waitFor(() => expect(goto).toHaveBeenCalledWith('/dashboard/new-id'));
	});

	it('sends an expired session to sign in', async () => {
		const { create, user } = setup();
		create.mockRejectedValue(new GalleryApiError(401, 'unauthorized'));

		await user.type(titleField(), 'Portraits');
		await submit(user);

		await waitFor(() =>
			expect(goto).toHaveBeenCalledWith('/login?expired=1', { refreshAll: true })
		);
	});

	it('reports another failed creation and stays open', async () => {
		const { create, user } = setup();
		create.mockRejectedValue(new GalleryApiError(500, 'boom'));

		await user.type(titleField(), 'Portraits');
		await submit(user);

		expect((await screen.findByRole('alert')).textContent).toMatch(/Couldn’t create/);
		expect(goto).not.toHaveBeenCalled();
	});

	it('closes on Escape and on Cancel', async () => {
		const { onclose, user } = setup();

		await user.keyboard('{Escape}');
		await user.click(screen.getByRole('button', { name: 'Cancel' }));

		expect(onclose).toHaveBeenCalledTimes(2);
	});

	it('reports a native close that bypasses Escape and Cancel', async () => {
		const { onclose, container } = setup();

		container.querySelector('dialog')?.dispatchEvent(new Event('close'));

		expect(onclose).toHaveBeenCalledTimes(1);
	});

	it('starts with focus in the title field and has no axe violations', async () => {
		const { container } = setup();

		expect(document.activeElement).toBe(titleField());
		expect((await axe(container)).violations).toEqual([]);
	});
});
