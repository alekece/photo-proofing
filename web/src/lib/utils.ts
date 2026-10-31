import { clsx, type ClassValue } from 'clsx';
import { extendTailwindMerge } from 'tailwind-merge';

// Teach the merger our custom radius tokens so `rounded-dialog` overrides `rounded-card`.
const twMerge = extendTailwindMerge({
	extend: { theme: { radius: ['card', 'control', 'dialog'] } }
});

export function cn(...inputs: ClassValue[]) {
	return twMerge(clsx(inputs));
}
export type WithElementRef<T, U extends HTMLElement = HTMLElement> = T & {
	ref?: U | null;
};
