<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { Lock } from '@lucide/svelte';
	import CoverBackdrop from '#lib/components/CoverBackdrop.svelte';
	import StudioSignature from '#lib/components/StudioSignature.svelte';
	import { GalleryApiError, unlockSharedGallery } from '#lib/gallery/api.js';
	import { formatFullDate } from '#lib/gallery/format.js';
	import type { LockedSharedGallery } from '#lib/gallery/types.js';
	import { DEFAULT_SIZE } from '#lib/settings/settings.js';

	const ACCESS_CODE_RE = /^[0-9]{6}$/;

	let {
		token,
		gallery,
		coverUrl,
		onunlock
	}: {
		token: string;
		gallery: LockedSharedGallery;
		coverUrl?: string;
		onunlock: () => Promise<void> | void;
	} = $props();

	let code = $state('');
	let submitting = $state(false);
	let errorMessage = $state('');

	const title = $derived(gallery.title ?? m.gallery_untitled());
	const formattedShootDate = $derived(
		gallery.shoot_date ? formatFullDate(`${gallery.shoot_date}T12:00`) : null
	);
	const studioLogo = $derived(
		gallery.has_logo ? `/api/v1/galleries/shared/${encodeURIComponent(token)}/logo` : null
	);

	async function handleSubmit(event: SubmitEvent) {
		event.preventDefault();
		if (submitting) return;

		if (!ACCESS_CODE_RE.test(code)) {
			errorMessage = m.client_gate_invalid_length();
			return;
		}

		submitting = true;
		errorMessage = '';

		try {
			await unlockSharedGallery(token, code);
			await onunlock();
		} catch (error) {
			if (error instanceof GalleryApiError) {
				if (error.status === 401) {
					errorMessage = m.client_gate_wrong_code();
				} else if (error.status === 429) {
					errorMessage = m.client_gate_too_many_attempts();
				} else {
					errorMessage = error.message;
				}
			} else {
				errorMessage = m.api_error_generic();
			}
		} finally {
			submitting = false;
		}
	}
</script>

<main
	class="relative isolate flex min-h-[max(520px,100dvh)] flex-col items-center justify-between overflow-hidden bg-[#16120f] px-5 py-7 text-center text-panel sm:px-8 sm:py-9"
>
	{#if coverUrl}
		<img
			src={coverUrl}
			alt={m.client_cover_alt()}
			class="absolute inset-0 -z-20 size-full object-cover brightness-[0.38]"
		/>
		<div
			data-image-context-menu-overlay
			class="absolute inset-0 -z-10 bg-[radial-gradient(circle_at_center,rgb(22_18_15/0.25),rgb(22_18_15/0.78)_78%)]"
		></div>
	{:else}
		<CoverBackdrop title={gallery.title} edge={formattedShootDate} centered />
	{/if}

	<StudioSignature
		studio={gallery.studio_name}
		logo={studioLogo}
		signature={gallery.signature ?? 'name'}
		size={gallery.signature_size ?? DEFAULT_SIZE}
		class="font-serif text-[1.1875rem] italic max-sm:text-[0.9375rem]"
	/>

	<div class="my-auto flex w-full max-w-md animate-photo-in flex-col items-center gap-4 py-8">
		<h1 class="font-serif text-[clamp(2.25rem,5vw,3.75rem)] leading-[1.06] wrap-anywhere">
			{title}
		</h1>
		{#if formattedShootDate && gallery.shoot_date}
			<p class="text-xs tracking-[0.2em] uppercase opacity-85">
				<time datetime={gallery.shoot_date}>{formattedShootDate}</time>
			</p>
		{/if}

		<form
			onsubmit={handleSubmit}
			novalidate
			class="mt-3 flex w-full max-w-xs flex-col items-center gap-3 rounded-[1.25rem] border border-panel/25 bg-[#16120f]/75 p-4 shadow-[0_18px_40px_-20px_rgb(0_0_0/0.65)] backdrop-blur-md sm:max-w-sm sm:p-5"
		>
			<p class="flex items-center gap-1.5 text-xs tracking-[0.16em] text-panel/90 uppercase">
				<Lock size={13} aria-hidden="true" />{m.client_gate_prompt()}
			</p>
			<div class="flex w-full items-center gap-2 max-[360px]:flex-col">
				<label for="client-access-code" class="sr-only">{m.client_gate_code_label()}</label>
				<input
					id="client-access-code"
					type="text"
					inputmode="numeric"
					pattern="[0-9]{6}"
					maxlength="6"
					autocomplete="one-time-code"
					placeholder="000000"
					disabled={submitting}
					value={code}
					oninput={(event) => {
						const digits = (event.currentTarget as HTMLInputElement).value
							.replace(/\D/g, '')
							.slice(0, 6);
						code = digits;
						(event.currentTarget as HTMLInputElement).value = digits;
						if (errorMessage) errorMessage = '';
					}}
					class="min-w-0 flex-1 rounded-full border border-panel/30 bg-[#16120f]/85 px-4 py-2.5 text-center font-mono text-base font-semibold tracking-[0.28em] text-panel placeholder:text-panel/35 focus-visible:outline-3 focus-visible:outline-offset-2 focus-visible:outline-focus disabled:opacity-65 max-[360px]:w-full"
				/>
				<button
					type="submit"
					disabled={submitting}
					class="inline-flex shrink-0 cursor-pointer items-center justify-center rounded-full border border-panel bg-panel px-5 py-2.5 text-xs font-bold tracking-[0.16em] text-foreground uppercase transition-colors hover:bg-panel/90 focus-visible:outline-3 focus-visible:outline-offset-2 focus-visible:outline-focus disabled:cursor-progress disabled:opacity-80 max-[360px]:w-full"
				>
					{submitting ? m.client_gate_submitting() : m.client_gate_submit()}
				</button>
			</div>
			{#if errorMessage}
				<p role="alert" class="text-xs font-medium text-warning-on-dark">{errorMessage}</p>
			{/if}
		</form>
	</div>

	<span aria-hidden="true"></span>
</main>
