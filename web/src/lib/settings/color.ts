import * as v from 'valibot';

export const rgbHexSchema = v.pipe(v.string(), v.length(7), v.regex(/^#[0-9A-F]{6}$/));
