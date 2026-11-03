/** Oldest first: the order screenshots of a scrolled list were taken in. */
export const sortByTaken = (files: File[]) =>
  [...files].sort(
    (a, b) =>
      a.lastModified - b.lastModified || a.name.localeCompare(b.name, undefined, { numeric: true }),
  );
