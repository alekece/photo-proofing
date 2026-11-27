<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { Contact } from '#lib/contacts/api.js';
	import { contactInitials } from '#lib/contacts/format.js';

	let {
		contact,
		showPhone = true,
		details
	}: {
		contact: Pick<Contact, 'name' | 'email' | 'phone'>;
		showPhone?: boolean;
		details?: Snippet;
	} = $props();
</script>

<div class="flex min-w-0 flex-1 items-center gap-2.5">
	<span
		aria-hidden="true"
		class="grid size-9 shrink-0 place-items-center rounded-full bg-border-strong/55 text-xs font-semibold tracking-wide text-foreground"
	>
		{contactInitials(contact.name)}
	</span>
	<div class="grid min-w-0 flex-1 gap-0.5">
		<p class="truncate text-sm font-semibold text-foreground">{contact.name}</p>
		<p class="truncate text-xs text-muted-foreground">{contact.email}</p>
		{#if showPhone && contact.phone}
			<p class="truncate text-xs text-muted-foreground">{contact.phone}</p>
		{/if}
		{@render details?.()}
	</div>
</div>
