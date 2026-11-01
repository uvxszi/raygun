import { Heart } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

/** Add to library / In library, as a pill under a video title's header. */
export const LibraryPill: FC<{ inLibrary: boolean; busy: boolean; onToggle: () => void }> = ({
  inLibrary,
  busy,
  onToggle,
}) => {
  const { t } = useTranslation();
  return (
    <button
      type="button"
      aria-pressed={inLibrary}
      disabled={busy}
      onClick={onToggle}
      className={`flex items-center gap-2 rounded-full px-4 py-2 text-sm disabled:opacity-60 ${inLibrary ? 'bg-primary/20 text-primary font-semibold' : 'bg-surface/80'}`}
    >
      <Heart aria-hidden size={16} fill={inLibrary ? 'currentColor' : 'none'} />
      {t(inLibrary ? 'manga.inLibrary' : 'manga.addToLibrary')}
    </button>
  );
};
