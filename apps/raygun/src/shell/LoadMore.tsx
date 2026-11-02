import { LoaderCircle } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useSettings } from '../settings/useSettings';

type LoadMoreProps = {
  hasNextPage: boolean;
  fetching: boolean;
  onLoad: () => void;
  label?: string;
};

/**
 * A list's next page (M46.8): loaded as the list's end comes near, at most once per the
 * cooldown in Settings → Browse so sources don't rate-limit; a Load more button when that's off.
 */
export const LoadMore: FC<LoadMoreProps> = ({ hasNextPage, fetching, onLoad, label }) => {
  const { t } = useTranslation();
  const prefs = useSettings().data?.autoLoad;
  const auto = (prefs?.enabled ?? true) && typeof IntersectionObserver !== 'undefined';
  const cooldown = prefs?.cooldownMs ?? 1500;
  const sentinel = useRef<HTMLDivElement>(null);
  const [near, setNear] = useState(false);
  const lastLoad = useRef(0);
  const loadRef = useRef(onLoad);
  useEffect(() => {
    loadRef.current = onLoad;
  });

  useEffect(() => {
    const element = sentinel.current;
    if (!auto || !element) return;
    const observer = new IntersectionObserver(
      (entries) => setNear(entries.some((entry) => entry.isIntersecting)),
      { rootMargin: '600px' },
    );
    observer.observe(element);
    return () => observer.disconnect();
  }, [auto, hasNextPage]);

  // Near the end, idle and allowed: the next page after the cooldown. A page too short to
  // move the marker away loads the one after it the same way.
  useEffect(() => {
    if (!auto || !near || !hasNextPage || fetching) return;
    const wait = Math.max(0, lastLoad.current + cooldown - Date.now());
    const timer = window.setTimeout(() => {
      lastLoad.current = Date.now();
      loadRef.current();
    }, wait);
    return () => window.clearTimeout(timer);
  }, [auto, near, hasNextPage, fetching, cooldown]);

  if (!hasNextPage) return null;
  if (auto) {
    return (
      <div ref={sentinel} className="flex justify-center py-3">
        {fetching && (
          <LoaderCircle aria-label={t('manga.loading')} className="text-muted animate-spin" />
        )}
      </div>
    );
  }
  return (
    <button
      type="button"
      disabled={fetching}
      onClick={onLoad}
      className="bg-surface flex items-center gap-2 self-center rounded-full px-5 py-2 text-sm disabled:opacity-60"
    >
      {fetching && <LoaderCircle aria-hidden size={16} className="animate-spin" />}
      {label ?? t('manga.loadMore')}
    </button>
  );
};
