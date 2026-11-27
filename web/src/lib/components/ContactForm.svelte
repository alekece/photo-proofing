<script module lang="ts">
	export type ContactDraft = {
		form: string;
		name: string;
		email: string;
		phone: string;
		notes: string;
		error: string;
		invalid: 'name' | 'email' | null;
	};
</script>

<script lang="ts">
	import type { Snippet } from 'svelte';
	import { m } from '#lib/paraglide/messages.js';
	import Input from './ui/Input/Input.svelte';
	import Label from './ui/Label/Label.svelte';

	type DraftChange = Partial<
		Pick<ContactDraft, 'name' | 'email' | 'phone' | 'notes' | 'error' | 'invalid'>
	>;

	let {
		draft,
		compact = false,
		onchange
	}: {
		draft: ContactDraft;
		compact?: boolean;
		onchange: (change: DraftChange) => void;
	} = $props();

	function markInvalid(field: 'name' | 'email', input: HTMLInputElement) {
		if (input.form?.querySelector(':invalid:not(fieldset)') !== input) return;

		onchange({
			invalid: field,
			error: field === 'name' ? m.contacts_name_required() : m.contacts_email_invalid()
		});
		input.focus();
	}
</script>

{#snippet field(
	name: 'name' | 'email',
	label: string,
	input: Snippet<[{ id: string; invalid: true | undefined; describedby: string | undefined }]>
)}
	{@const invalid = draft.invalid === name}
	<div class="grid gap-1.5">
		<Label for="{draft.form}-{name}" class="after:ml-0.5 after:text-danger after:content-['*']">
			{label}
		</Label>
		{@render input({
			id: `${draft.form}-${name}`,
			invalid: invalid || undefined,
			describedby: invalid ? `${draft.form}-error` : undefined
		})}
		{#if invalid && draft.error}
			<p id="{draft.form}-error" role="alert" class="text-sm text-danger">{draft.error}</p>
		{/if}
	</div>
{/snippet}

{#snippet phone()}
	<div class="grid gap-1.5 {compact ? 'sm:max-w-xs' : ''}">
		<Label for="{draft.form}-phone">{m.contacts_phone()}</Label>
		<Input
			id="{draft.form}-phone"
			type="tel"
			value={draft.phone}
			maxlength={40}
			oninput={(event) => onchange({ phone: event.currentTarget.value })}
		/>
	</div>
{/snippet}

<div
	class={compact
		? 'grid min-w-0 items-start gap-3 sm:grid-cols-2 lg:grid-cols-4'
		: 'grid min-w-0 gap-4'}
>
	<div class={compact ? 'contents' : 'grid gap-4'}>
		{#snippet nameInput(a: {
			id: string;
			invalid: true | undefined;
			describedby: string | undefined;
		})}
			<Input
				id={a.id}
				value={draft.name}
				maxlength={120}
				required
				aria-invalid={a.invalid}
				aria-describedby={a.describedby}
				oninput={(event) => onchange({ name: event.currentTarget.value })}
				oninvalid={(event) => {
					event.preventDefault();
					markInvalid('name', event.currentTarget);
				}}
			/>
		{/snippet}
		{#snippet emailInput(a: {
			id: string;
			invalid: true | undefined;
			describedby: string | undefined;
		})}
			<Input
				id={a.id}
				type="email"
				value={draft.email}
				maxlength={254}
				required
				aria-invalid={a.invalid}
				aria-describedby={a.describedby}
				oninput={(event) => onchange({ email: event.currentTarget.value })}
				oninvalid={(event) => {
					event.preventDefault();
					markInvalid('email', event.currentTarget);
				}}
			/>
		{/snippet}
		{@render field('name', m.common_name(), nameInput)}
		{@render field('email', m.login_email(), emailInput)}
		{#if !compact}{@render phone()}{/if}
	</div>
	{#if compact}{@render phone()}{/if}
	<div class="grid gap-1.5">
		<Label for="{draft.form}-notes">{m.contacts_notes()}</Label>
		<textarea
			id="{draft.form}-notes"
			value={draft.notes}
			rows={compact ? 1 : 4}
			maxlength={500}
			oninput={(event) => onchange({ notes: event.currentTarget.value })}
			class="cn-input h-auto w-full min-w-0 resize-y placeholder:text-muted-foreground focus-visible:border-foreground disabled:cursor-not-allowed disabled:opacity-50"
		></textarea>
	</div>
</div>
{#if draft.error && !draft.invalid}
	<p id="{draft.form}-error" role="alert" class="mt-3 text-sm text-danger">{draft.error}</p>
{/if}
