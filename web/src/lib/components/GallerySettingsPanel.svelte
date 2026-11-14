<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { onDestroy } from 'svelte';
	import { Archive, ArchiveRestore, Check, Link2, RotateCw, Trash2 } from '@lucide/svelte';
	import ClickPop from '#lib/components/ClickPop.svelte';
	import ShareLink from '#lib/components/ShareLink.svelte';
	import Button from '#lib/components/ui/Button/Button.svelte';
	import Card from '#lib/components/ui/Card/Card.svelte';
	import DatePicker from '#lib/components/ui/DatePicker/DatePicker.svelte';
	import Input from '#lib/components/ui/Input/Input.svelte';
	import Label from '#lib/components/ui/Label/Label.svelte';
	import Switch from '#lib/components/ui/Switch/Switch.svelte';
	import { toasts } from '#lib/components/ui/Toast/toasts.svelte.js';
	import type { Gallery, GalleryChanges } from '#lib/gallery/api.js';
	import { endOfLocalDay, toLocalDate } from '#lib/gallery/format.js';

	/**
	 * Studio `Settings` view, split into three Warm Paper cards:
	 * 1. General (`Shoot date`, `Included photos`, `Open until`)
	 * 2. Privacy & Protection (`Client link`, `Enable watermarks`)
	 * 3. Actions (`Archive` / `Restore`, `Delete`)
	 */
	let {
		gallery,
		shareToken,
		origin,
		expectedAccessCode = $bindable(),
		onsave,
		schedule,
		cancel,
		onsharelink,
		onarchive,
		onrestore,
		ondelete
	}: {
		gallery: Gallery;
		shareToken: string | null;
		origin: string;
		/** Desired pre-share access code (`string`, `null`, or `'invalid'` while incomplete). */
		expectedAccessCode?: string | null | 'invalid';
		onsave: (changes: GalleryChanges) => Promise<boolean>;
		schedule: (field: string, run: () => void) => void;
		cancel: (field: string) => void;
		onsharelink?: () => void;
		onarchive: () => void;
		onrestore: () => void;
		ondelete: () => void;
	} = $props();

	const savedExpiry = $derived(gallery.expires_at ? toLocalDate(gallery.expires_at) : '');
	const savedShootDate = $derived(gallery.shoot_date);
	const savedIncluded = $derived(gallery.included_photos);
	const savedWatermark = $derived(gallery.watermark_enabled);
	const savedAccessCode = $derived(gallery.access_code ?? null);

	let expiryDay = $derived(savedExpiry);
	let shootDate = $derived(savedShootDate);
	let watermark = $derived(savedWatermark);
	// svelte-ignore state_referenced_locally
	let included = $state<number | null>(savedIncluded);
	let includedDirty = false;
	// svelte-ignore state_referenced_locally
	let lastSavedIncluded = savedIncluded;

	const ACCESS_CODE_RE = /^[0-9]{6}$/;

	// svelte-ignore state_referenced_locally
	let committedAccessCode = $state<string | null>(savedAccessCode);
	// svelte-ignore state_referenced_locally
	let desiredAccessCode = $state<string | null>(savedAccessCode);
	const lockedAccessCode = $derived(savedAccessCode ?? committedAccessCode);
	// svelte-ignore state_referenced_locally
	let accessCodeEnabled = $state(savedAccessCode !== null);
	// svelte-ignore state_referenced_locally
	let accessCodeDraft = $state(savedAccessCode ?? '');
	let accessCodeDirty = false;
	let accessCodeSaving = false;
	// svelte-ignore state_referenced_locally
	let lastPropAccessCode = savedAccessCode;
	let codeCopied = $state(false);
	let codePopping = $state(false);
	let codeCopyFailed = $state(false);
	let codeInput = $state<HTMLInputElement | null>(null);
	let copyTimer: ReturnType<typeof setTimeout> | undefined;

	onDestroy(() => clearTimeout(copyTimer));

	$effect.pre(() => {
		if (savedIncluded !== lastSavedIncluded) {
			lastSavedIncluded = savedIncluded;
			if (!includedDirty) included = savedIncluded;
		}
		if (savedAccessCode !== lastPropAccessCode) {
			lastPropAccessCode = savedAccessCode;
			committedAccessCode = savedAccessCode;
			if (!accessCodeSaving && !accessCodeDirty) {
				desiredAccessCode = savedAccessCode;
				accessCodeEnabled = savedAccessCode !== null;
				accessCodeDraft = savedAccessCode ?? '';
			}
		}
		expectedAccessCode = accessCodeEnabled
			? ACCESS_CODE_RE.test(accessCodeDraft)
				? accessCodeDraft
				: 'invalid'
			: null;
	});

	const archived = $derived(gallery.states.includes('archived'));
	const delivered = $derived(gallery.states.includes('delivered'));

	const card = 'rounded-[0.875rem] border border-border bg-panel p-4 sm:p-5';
	const heading =
		'mb-2.5 font-sans! text-[0.6875rem] leading-none font-semibold! tracking-[0.08em] text-muted-foreground uppercase';
	const row =
		'flex flex-wrap items-center justify-between gap-2.5 border-t border-[#ece3d6] py-3 text-sm last:pb-0 [h3+&]:border-t-0 [h3+&]:pt-0.5';

	async function commit(changes: GalleryChanges, revert: () => void) {
		if (!(await onsave(changes))) revert();
	}

	function setShootDate(day: string | null) {
		const previous = shootDate;

		shootDate = day;
		void commit({ shoot_date: day }, () => (shootDate = previous));
	}

	function setExpiry(day: string | null) {
		const next = day ?? '';

		expiryDay = next;
		void commit(
			{ expires_at: next ? endOfLocalDay(next) : null },
			() => (expiryDay = gallery.expires_at ? toLocalDate(gallery.expires_at) : '')
		);
	}

	/** While typing, a half-typed value is left alone; leaving the field is where it is judged. */
	function saveIncluded(leaving: boolean) {
		const count = included;
		const revert = () => {
			if (leaving || included === count) {
				includedDirty = false;
				included = gallery.included_photos;
			}
		};

		if (count !== null && (!Number.isInteger(count) || count < 1)) {
			if (leaving) {
				toasts.error(m.settings_included_invalid());
				revert();
			}

			return;
		}
		if (count === gallery.included_photos) {
			if (leaving) includedDirty = false;

			return;
		}

		void (async () => {
			if (await onsave({ included_photos: count })) {
				if (included === count) includedDirty = false;
			} else {
				revert();
			}
		})();
	}

	function setWatermark(value: boolean) {
		void commit({ watermark_enabled: value }, () => (watermark = !value));
	}

	function generateRandomAccessCode(): string {
		const limit = 4_294_000_000;
		const buffer = new Uint32Array(1);
		let sample = limit;

		while (sample >= limit) {
			crypto.getRandomValues(buffer);
			sample = buffer[0]!;
		}

		return String(sample % 1_000_000).padStart(6, '0');
	}

	function revertAccessCode() {
		accessCodeDirty = false;
		desiredAccessCode = committedAccessCode;
		accessCodeEnabled = committedAccessCode !== null;
		accessCodeDraft = committedAccessCode ?? '';
	}

	function requestAccessCodeSave(target: string | null) {
		desiredAccessCode = target;
		if (accessCodeSaving) return;
		void pumpAccessCodeSave();
	}

	async function pumpAccessCodeSave() {
		accessCodeSaving = true;
		try {
			while (desiredAccessCode !== committedAccessCode) {
				const target = desiredAccessCode;
				const ok = await onsave({ access_code: target });

				if (ok) {
					committedAccessCode = target;
					if (desiredAccessCode === target && !accessCodeDirty) {
						accessCodeEnabled = target !== null;
						accessCodeDraft = target ?? '';
					}
				} else {
					if (desiredAccessCode === target) {
						revertAccessCode();
					}
					return;
				}
			}
		} finally {
			accessCodeSaving = false;
		}
	}

	function toggleAccessCode(enabled: boolean) {
		cancel('access_code');
		accessCodeDirty = false;

		if (!enabled) {
			accessCodeDraft = '';
			requestAccessCodeSave(null);
			return;
		}

		const next = ACCESS_CODE_RE.test(accessCodeDraft)
			? accessCodeDraft
			: generateRandomAccessCode();
		accessCodeDraft = next;
		requestAccessCodeSave(next);
	}

	function regenerateAccessCode() {
		cancel('access_code');
		accessCodeDirty = false;

		const next = generateRandomAccessCode();
		accessCodeDraft = next;
		requestAccessCodeSave(next);
	}

	function saveAccessCode(leaving: boolean, event?: FocusEvent) {
		const candidate = accessCodeDraft;

		if (!ACCESS_CODE_RE.test(candidate)) {
			if (leaving) {
				const clickingShare =
					event?.relatedTarget instanceof Element &&
					event.relatedTarget.closest('[data-share-trigger]') !== null;

				if (!clickingShare) {
					toasts.error(m.settings_access_code_invalid());
					revertAccessCode();
				}
			}
			return;
		}

		accessCodeDirty = false;
		if (candidate === desiredAccessCode && !accessCodeSaving && candidate === committedAccessCode) {
			return;
		}

		requestAccessCodeSave(candidate);
	}

	async function copyAccessCode(code: string) {
		if (!ACCESS_CODE_RE.test(code)) return;
		clearTimeout(copyTimer);
		try {
			await navigator.clipboard.writeText(code);
			codeCopyFailed = false;
			codeCopied = true;
			codePopping = true;
			copyTimer = setTimeout(() => (codeCopied = false), 3000);
		} catch {
			codeCopied = false;
			codeCopyFailed = true;
			codeInput?.focus();
			codeInput?.select();
		}
	}
</script>

<div class="grid max-w-2xl gap-4">
	<h2 class="text-2xl leading-tight">{m.tab_settings()}</h2>

	<Card as="section" class={card}>
		<h3 class={heading}>{m.settings_general()}</h3>
		<div class={row}>
			<Label for="shoot-date" class="shrink-0 text-foreground">{m.shoot_date_label()}</Label>
			<DatePicker
				id="shoot-date"
				placeholder="–"
				align="end"
				rootClass="w-44 min-w-0"
				class="min-h-0 w-full rounded-lg bg-white px-2.5 py-1.5 text-sm font-semibold [&>span]:flex-1 [&>span]:text-center"
				value={shootDate}
				clearable
				onchange={setShootDate}
			/>
		</div>
		<div class={row}>
			<Label for="included" class="text-foreground">{m.settings_included()}</Label>
			<Input
				id="included"
				placeholder="–"
				type="number"
				min="1"
				step="1"
				class="min-h-0 w-[4.75rem] [appearance:textfield] rounded-lg bg-white px-2 py-1.5 text-center text-sm font-semibold placeholder:text-muted-foreground/40 [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none"
				bind:value={included}
				disabled={delivered}
				oninput={() => {
					includedDirty = true;
					schedule('included_photos', () => saveIncluded(false));
				}}
				onblur={() => {
					cancel('included_photos');
					saveIncluded(true);
				}}
			/>
		</div>
		<div class={row}>
			<Label for="expiry" class="shrink-0 text-foreground">{m.expiry_open_until()}</Label>
			<DatePicker
				id="expiry"
				placeholder="–"
				align="end"
				presets
				rootClass="w-44 min-w-0"
				class="min-h-0 w-full rounded-lg bg-white px-2.5 py-1.5 text-sm font-semibold [&>span]:flex-1 [&>span]:text-center"
				value={expiryDay || null}
				clearable
				onchange={setExpiry}
			/>
		</div>
	</Card>

	<Card as="section" class={card}>
		<h3 class={heading}>{m.settings_privacy_protection()}</h3>
		<div class={row}>
			<span class="text-foreground">{m.copy_link_label()}</span>
			{#if shareToken}
				<div class="w-full sm:w-auto sm:min-w-64">
					<ShareLink token={shareToken} {origin} />
				</div>
			{:else}
				<div class="flex flex-wrap items-center gap-2.5">
					<span class="text-sm text-muted-foreground">{m.settings_not_shared()}</span>
					<Button
						data-share-trigger
						variant="outline"
						size="sm"
						class="min-h-9! gap-1.5 bg-white!"
						disabled={archived}
						onclick={onsharelink}
					>
						<Link2 size={15} aria-hidden="true" />{m.settings_share_link()}
					</Button>
				</div>
			{/if}
		</div>
		{#if shareToken}
			{#if lockedAccessCode !== null}
				<div class={row}>
					<div class="grid gap-0.5">
						<span class="text-foreground">{m.settings_access_code()}</span>
						<span class="text-xs text-muted-foreground">{m.settings_access_code_locked_hint()}</span
						>
					</div>
					<div class="grid gap-1.5">
						<div class="flex items-center gap-2">
							<Input
								bind:ref={codeInput}
								readonly
								value={lockedAccessCode}
								aria-label={m.settings_access_code_label()}
								class="bg-muted/40 min-h-0 w-28 cursor-default rounded-lg border-border px-2.5 py-1.5 text-center font-mono text-sm font-semibold tracking-[0.18em] text-foreground focus-visible:outline-none"
							/>
							<Button
								size="sm"
								class="min-h-9! min-w-24 gap-1.5"
								onclick={() => copyAccessCode(lockedAccessCode ?? '')}
							>
								{#if codeCopied}
									<span class="text-ok-on-dark">
										<ClickPop active={codePopping} onend={() => (codePopping = false)}>
											<Check size={16} aria-hidden="true" />
										</ClickPop>
									</span>
									{m.copy_copied()}
								{:else}
									{m.copy_copy()}
								{/if}
							</Button>
						</div>
						<p class={codeCopyFailed ? 'text-xs text-danger' : 'sr-only'} role="status">
							{codeCopied ? m.copy_copied() : codeCopyFailed ? m.copy_failed() : ''}
						</p>
					</div>
				</div>
			{:else}
				<div class={row}>
					<div class="grid gap-0.5">
						<Label for="access-code-switch" class="text-foreground"
							>{m.settings_access_code()}</Label
						>
						<span class="text-xs text-muted-foreground">{m.settings_access_code_none_locked()}</span
						>
					</div>
					<Switch id="access-code-switch" checked={false} disabled />
				</div>
			{/if}
		{:else}
			<div class={row}>
				<Label for="access-code-switch" class="text-foreground">{m.settings_access_code()}</Label>
				<Switch
					id="access-code-switch"
					bind:checked={accessCodeEnabled}
					onchange={toggleAccessCode}
				/>
				{#if accessCodeEnabled}
					<div class="mt-1 flex basis-full flex-wrap items-center justify-end gap-2">
						<Input
							id="access-code-input"
							bind:ref={codeInput}
							type="text"
							inputmode="numeric"
							pattern="[0-9]{6}"
							maxlength={6}
							placeholder="000000"
							aria-label={m.settings_access_code_label()}
							value={accessCodeDraft}
							oninput={(event) => {
								const digits = (event.currentTarget as HTMLInputElement).value
									.replace(/\D/g, '')
									.slice(0, 6);
								accessCodeDraft = digits;
								(event.currentTarget as HTMLInputElement).value = digits;
								accessCodeDirty = true;
								if (ACCESS_CODE_RE.test(digits)) {
									schedule('access_code', () => saveAccessCode(false));
								} else {
									cancel('access_code');
								}
							}}
							onblur={(event) => {
								cancel('access_code');
								saveAccessCode(true, event);
							}}
							class="min-h-0 w-28 rounded-lg bg-white px-2.5 py-1.5 text-center font-mono text-sm font-semibold tracking-[0.18em] text-foreground placeholder:text-muted-foreground/40"
						/>
						<Button
							variant="outline"
							size="sm"
							class="min-h-9! shrink-0 bg-white!"
							onclick={regenerateAccessCode}
						>
							<RotateCw
								size={15}
								aria-hidden="true"
								class="mr-1.5"
							/>{m.settings_access_code_generate()}
						</Button>
						<Button
							size="sm"
							class="min-h-9! min-w-24 gap-1.5"
							disabled={!ACCESS_CODE_RE.test(accessCodeDraft)}
							onclick={() => copyAccessCode(accessCodeDraft)}
						>
							{#if codeCopied}
								<span class="text-ok-on-dark">
									<ClickPop active={codePopping} onend={() => (codePopping = false)}>
										<Check size={16} aria-hidden="true" />
									</ClickPop>
								</span>
								{m.copy_copied()}
							{:else}
								{m.copy_copy()}
							{/if}
						</Button>
						<p
							class={codeCopyFailed ? 'basis-full text-right text-xs text-danger' : 'sr-only'}
							role="status"
						>
							{codeCopied ? m.copy_copied() : codeCopyFailed ? m.copy_failed() : ''}
						</p>
					</div>
				{/if}
			</div>
		{/if}
		<div class={row}>
			<Label for="watermark" class="text-foreground">{m.settings_watermark()}</Label>
			<Switch id="watermark" bind:checked={watermark} onchange={setWatermark} />
		</div>
	</Card>

	<Card as="section" class={card}>
		<h3 class={heading}>{m.settings_actions()}</h3>
		<div class={row}>
			<div class="grid gap-0.5">
				<span class="font-medium text-foreground"
					>{archived ? m.action_restore() : m.action_archive()}</span
				>
				<span class="text-xs text-muted-foreground"
					>{archived ? m.settings_restore_hint() : m.settings_archive_hint()}</span
				>
			</div>
			{#if archived}
				<Button variant="outline" size="sm" class="min-h-9! shrink-0 bg-white!" onclick={onrestore}>
					<ArchiveRestore size={16} aria-hidden="true" class="mr-1.5" />{m.action_restore()}
				</Button>
			{:else}
				<Button variant="outline" size="sm" class="min-h-9! shrink-0 bg-white!" onclick={onarchive}>
					<Archive size={16} aria-hidden="true" class="mr-1.5" />{m.action_archive()}
				</Button>
			{/if}
		</div>
		<div class={row}>
			<div class="grid gap-0.5">
				<span class="font-medium text-foreground">{m.action_delete()}</span>
				<span class="text-xs text-muted-foreground">{m.settings_delete_hint()}</span>
			</div>
			<Button
				variant="outline"
				size="sm"
				class="min-h-9! shrink-0 border-danger/35! bg-white! text-danger!"
				onclick={ondelete}
			>
				<Trash2 size={16} aria-hidden="true" class="mr-1.5" />{m.action_delete()}
			</Button>
		</div>
	</Card>
</div>
