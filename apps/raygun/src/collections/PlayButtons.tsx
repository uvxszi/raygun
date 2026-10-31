import { Play, Shuffle } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

type PlayButtonsProps = {
  onPlay: (shuffle: boolean) => void;
  disabled?: boolean;
};

export const PlayButtons: FC<PlayButtonsProps> = ({ onPlay, disabled = false }) => {
  const { t } = useTranslation();
  return (
    <div className="flex gap-3">
      <button
        type="button"
        disabled={disabled}
        onClick={() => onPlay(false)}
        className="bg-primary text-background flex items-center gap-2 rounded-full px-6 py-2.5 font-semibold disabled:opacity-50"
      >
        <Play aria-hidden size={18} />
        {t('library.play')}
      </button>
      <button
        type="button"
        disabled={disabled}
        onClick={() => onPlay(true)}
        className="bg-surface flex items-center gap-2 rounded-full px-6 py-2.5 disabled:opacity-50"
      >
        <Shuffle aria-hidden size={18} />
        {t('library.shuffle')}
      </button>
    </div>
  );
};
