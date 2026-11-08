<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import Button from '#lib/components/ui/Button/Button.svelte';
	import Dialog from '#lib/components/ui/Dialog/Dialog.svelte';
	import Input from '#lib/components/ui/Input/Input.svelte';
	import Label from '#lib/components/ui/Label/Label.svelte';
	import { endOfLocalDay, formatShortDate, toLocalDate } from '#lib/gallery/format.js';

	/**
	 * Offered after restoring a gallery that is still expired. `onsave` returns whether the new
	 * expiry was saved; the page reports failures itself, so a refusal just keeps the dialog open.
	 */
	type Props = {
		/** When the link expired, as an ISO date. */
		expiresAt: string;
		onsave: (expiresAt: string | null) => Promise<boolean>;
		onclose: () => void;
	};

	let { expiresAt, onsave, onclose }: Props = $props();

	const id = $props.id();
	const today = toLocalDate(new Date().toISOString());

	let day = $state('');
	let error = $state('');
	let pending = $state(false);
	let input = $state<HTMLInputElement | null>(null);

	$effect(() => input?.focus());

	async function save(value: string | null, close: () => void) {
		if (pending) return;

		if (value !== null && value < today) {
			error = m.restore_date_past();

			return;
		}

		error = '';
		pending = true;

		try {
			if (await onsave(value === null ? null : endOfLocalDay(value))) close();
		} finally {
			pending = false;
		}
	}
</script>

<Dialog
	{onclose}
	labelledby="{id}-title"
	class="m-auto w-[min(26rem,calc(100vw-2rem))] rounded-dialog border border-border bg-panel p-6 text-foreground shadow-xl"
>
	{#snippet children(close)}
		<form
			novalidate
			class="grid gap-4"
			aria-busy={pending}
			onsubmit={(event) => {
				event.preventDefault();

				if (day) void save(day, close);
			}}
		>
			<div class="grid gap-2">
				<h2 id="{id}-title" class="text-xl">{m.restore_title()}</h2>
				<p class="text-sm text-muted-foreground">
					{m.restore_body({ date: formatShortDate(expiresAt) })}
				</p>
			</div>
			<div class="grid gap-1.5">
				<Label for="{id}-day" class="text-muted-foreground">{m.expiry_open_until()}</Label>
				<Input
					id="{id}-day"
					type="date"
					min={today}
					bind:ref={input}
					bind:value={day}
					aria-invalid={error ? true : undefined}
					aria-describedby={error ? `${id}-error` : undefined}
				/>
				{#if error}<p id="{id}-error" role="alert" class="text-sm text-danger">{error}</p>{/if}
			</div>
			<div class="flex flex-wrap justify-end gap-2">
				<Button variant="ghost" type="button" onclick={close}>{m.restore_keep_expired()}</Button>
				<Button
					variant="outline"
					type="button"
					disabled={pending}
					onclick={() => void save(null, close)}>{m.restore_no_expiry()}</Button
				>
				<Button type="submit" disabled={pending || !day}>{m.restore_set_date()}</Button>
			</div>
		</form>
	{/snippet}
</Dialog>
