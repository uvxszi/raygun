/**
 * The resolver's key for a song (M46.36), as `raygun_resolver::track_key` makes it: the normalized
 * artist and the title without brackets or `feat.` credits. Downloads, favorites and remembered
 * picks are keyed by it.
 */
const normalize = (text: string) =>
  text
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '')
    .toLowerCase()
    .replaceAll('&', ' and ')
    .split(/[^\p{Alphabetic}\p{N}]+/u)
    .filter(Boolean)
    .join(' ');

const coreTitle = (title: string) => {
  let plain = '';
  let depth = 0;
  for (const char of title) {
    if (char === '(' || char === '[' || char === '{') depth += 1;
    else if (char === ')' || char === ']' || char === '}') depth = Math.max(0, depth - 1);
    else if (depth === 0) plain += char;
  }
  const words = normalize(plain).split(' ');
  const cut = words.findIndex((word) => word === 'feat' || word === 'ft' || word === 'featuring');
  return (cut === -1 ? words : words.slice(0, cut)).join(' ');
};

export const trackKey = (title: string, artist: string | null | undefined) =>
  `${normalize(artist ?? '')}|${coreTitle(title)}`;
