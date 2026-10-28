const HUES = [210, 150, 30, 280, 340, 90];

/** Generates an inline SVG data URI so no photo assets are committed. */
export function placeholderImage(label: string, seed: number, landscape = true): string {
	const [w, h] = landscape ? [1200, 800] : [800, 1200];
	const hue = HUES[seed % HUES.length];
	const svg =
		`<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}">` +
		`<rect width="100%" height="100%" fill="hsl(${hue} 35% 55%)"/>` +
		`<circle cx="${w * 0.7}" cy="${h * 0.3}" r="${h * 0.12}" fill="hsl(${hue} 60% 85%)"/>` +
		`<path d="M0 ${h} L${w * 0.35} ${h * 0.55} L${w * 0.6} ${h * 0.8} L${w * 0.8} ${h * 0.6} L${w} ${h} Z" fill="hsl(${hue} 40% 30%)"/>` +
		`<text x="50%" y="12%" text-anchor="middle" font-family="sans-serif" font-size="${h * 0.06}" fill="white">${label}</text>` +
		`</svg>`;
	return `data:image/svg+xml;utf8,${encodeURIComponent(svg)}`;
}
