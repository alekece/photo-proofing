import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { axe } from 'vitest-axe';
import DialogHarness from './dialog-harness.test.svelte';

const dialog = () => document.querySelector('dialog') as HTMLDialogElement;

describe('Dialog', () => {
	it('opens as a labelled dialog', async () => {
		const { container } = render(DialogHarness, { onclose: () => {} });

		expect(dialog().hasAttribute('open')).toBe(true);
		expect(screen.getByRole('dialog', { name: 'Example' })).toBeTruthy();
		expect((await axe(container)).violations).toEqual([]);
	});

	it('calls onclose once on Escape', async () => {
		const onclose = vi.fn();

		render(DialogHarness, { onclose });
		// jsdom's showModal fallback doesn't move focus into the dialog like browsers do.
		screen.getByRole('button', { name: 'Cancel' }).focus();
		await userEvent.keyboard('{Escape}');

		expect(onclose).toHaveBeenCalledOnce();
	});

	it('calls onclose once when content closes it', async () => {
		const onclose = vi.fn();

		render(DialogHarness, { onclose });
		await userEvent.click(screen.getByRole('button', { name: 'Cancel' }));

		expect(onclose).toHaveBeenCalledOnce();
	});

	it('calls onclose once for a native close event', () => {
		const onclose = vi.fn();

		render(DialogHarness, { onclose });
		dialog().dispatchEvent(new Event('close'));

		expect(onclose).toHaveBeenCalledOnce();
	});

	it('returns focus to the opener on unmount', async () => {
		render(DialogHarness, { onclose: () => {}, opened: false });
		const user = userEvent.setup();
		const opener = screen.getByRole('button', { name: 'Open' });

		await user.click(opener);
		expect(document.querySelector('dialog')).not.toBeNull();
		await user.click(screen.getByRole('button', { name: 'Unmount' }));

		expect(document.querySelector('dialog')).toBeNull();
		expect(document.activeElement).toBe(opener);
	});

	it('opens non-modally when asked to', () => {
		const showModal = vi.fn();
		const show = vi.fn();
		const proto = HTMLDialogElement.prototype;
		proto.showModal = showModal;
		proto.show = show;

		try {
			render(DialogHarness, { onclose: () => {}, modal: false });

			expect(show).toHaveBeenCalledTimes(1);
			expect(showModal).not.toHaveBeenCalled();
		} finally {
			// @ts-expect-error jsdom has neither method; restore that
			delete proto.showModal;
			// @ts-expect-error jsdom has neither method; restore that
			delete proto.show;
		}
	});
});
