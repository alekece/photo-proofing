/** The deployed frontend's build version: today the full commit SHA, later the release version. */
export const buildVersion: string = import.meta.env.PP_BUILD_SHA;

/** Shortens a commit SHA for display; any other version string is shown as is. */
export function displayVersion(version: string): string {
	return /^[0-9a-f]{40}$/.test(version) ? version.slice(0, 7) : version;
}
