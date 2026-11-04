import { Cpu } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { Sheet } from './Sheet';

/**
 * Smart Chips' button, right of the mascot's: small on-device models that will work on the
 * library in the background (sorting, transcribing, finding songs to like) are coming, and for
 * now it says so.
 */
export const SmartChipsButton: FC<{ className: string; iconSize: number }> = ({
  className,
  iconSize,
}) => {
  const { t } = useTranslation();
  const [anchor, setAnchor] = useState<HTMLButtonElement | null>(null);
  return (
    <>
      <button
        type="button"
        aria-label={t('smartChips.label')}
        data-hint={t('smartChips.label')}
        aria-haspopup="dialog"
        aria-expanded={anchor !== null}
        onClick={(event) => setAnchor(anchor ? null : event.currentTarget)}
        className={className}
      >
        <Cpu aria-hidden size={iconSize} />
      </button>
      {anchor && (
        <Sheet
          title={t('smartChips.label')}
          anchor={anchor}
          popoverWidth={280}
          onClose={() => setAnchor(null)}
        >
          <div data-testid="smart-chips-soon" className="flex items-center gap-3 px-3 pb-3">
            <span className="bg-primary/15 text-primary grid size-12 shrink-0 place-items-center rounded-full">
              <Cpu aria-hidden size={26} />
            </span>
            <span className="flex flex-col gap-0.5">
              <span className="font-semibold">{t('smartChips.soon')}</span>
              <span className="text-muted text-sm">{t('smartChips.what')}</span>
            </span>
          </div>
        </Sheet>
      )}
    </>
  );
};
