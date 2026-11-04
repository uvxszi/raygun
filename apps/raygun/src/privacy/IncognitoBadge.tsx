import { HatGlasses } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useIsDesktop } from '../shell/useIsDesktop';
import { useIncognito } from './useIncognito';

type IncognitoBadgeProps = {
  /** Shows the word beside the mask (the desktop bar). */
  labelled?: boolean;
  /** Left out on desktop, whose app bar has its own (a screen's top bar). */
  phoneOnly?: boolean;
  className?: string;
};

/** Shows that incognito is on (M47) while it is; a tap turns it off. */
export const IncognitoBadge: FC<IncognitoBadgeProps> = ({
  labelled,
  phoneOnly,
  className = '',
}) => {
  const { t } = useTranslation();
  const incognito = useIncognito();
  const desktop = useIsDesktop();
  if (!incognito.on || (phoneOnly && desktop)) return null;
  // A finger-sized target around a small pill.
  return (
    <button
      type="button"
      aria-label={t('incognito.turnOff')}
      data-hint={t('incognito.turnOff')}
      disabled={incognito.busy}
      onClick={() => incognito.set(false)}
      className={`group grid h-10 shrink-0 place-items-center disabled:opacity-60 ${className}`}
    >
      <span className="bg-foreground text-background flex h-8 items-center gap-1.5 rounded-full px-2.5 text-xs font-semibold transition-opacity group-hover:opacity-85">
        <HatGlasses aria-hidden size={16} />
        {labelled && <span>{t('incognito.title')}</span>}
      </span>
    </button>
  );
};
