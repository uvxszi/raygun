import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import type { PictureBox } from '../pictureBox';
import { shownBox } from './watermarkBox';
import type { Box } from './watermarkBox';

type Props = {
  picture: PictureBox | null;
  /** What Find it found. */
  box: Box;
  /** Remove (true) or Not this (false). */
  onAnswer: (remove: boolean) => void;
};

/**
 * What Find it found, outlined over the picture with a question: only Remove turns the remover
 * on; Not this leaves the title as it was.
 */
export const WatermarkProposal: FC<Props> = ({ picture, box, onAnswer }) => {
  const { t } = useTranslation();
  const rect = picture ? shownBox(box, picture) : null;
  return (
    <div
      role="dialog"
      aria-label={t('videoPlayer.watermarkAsk')}
      className="absolute inset-0 z-30"
      onClick={(event) => event.stopPropagation()}
    >
      {rect && (
        <div
          data-testid="watermark-proposal-box"
          className="border-primary pointer-events-none absolute rounded-sm border-2 shadow-[0_0_0_1px_rgba(0,0,0,0.6)]"
          style={{ left: rect.left, top: rect.top, width: rect.width, height: rect.height }}
        />
      )}
      <div className="absolute inset-x-0 top-6 flex justify-center">
        <div className="flex items-center gap-3 rounded-full bg-black/75 px-4 py-2 text-sm text-white">
          <span>{t('videoPlayer.watermarkAsk')}</span>
          <button
            type="button"
            className="bg-primary text-background rounded-full px-3 py-1 font-semibold"
            onClick={() => onAnswer(true)}
          >
            {t('videoPlayer.watermarkRemoveIt')}
          </button>
          <button
            type="button"
            className="rounded-full bg-white/15 px-3 py-1 font-semibold"
            onClick={() => onAnswer(false)}
          >
            {t('videoPlayer.watermarkNotThis')}
          </button>
        </div>
      </div>
    </div>
  );
};
