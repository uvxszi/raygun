import { Link } from '@tanstack/react-router';
import { Cpu } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { ChipList } from '../chips/ChipList';
import { Sheet } from './Sheet';

/**
 * Smart Chips' button, right of the mascot's: the list of Smart Chips (M46.57), optional features
 * installed like extensions, with their downloads and switches, and a link to their Settings page.
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
          popoverWidth={360}
          onClose={() => setAnchor(null)}
        >
          <div data-testid="smart-chips-list" className="flex flex-col pb-2">
            <p className="text-muted px-3 pb-1 text-sm">{t('smartChips.what')}</p>
            <ChipList />
            <Link
              to="/settings/chips"
              onClick={() => setAnchor(null)}
              className="text-primary self-start px-3 py-2 text-sm font-medium"
            >
              {t('smartChips.inSettings')}
            </Link>
          </div>
        </Sheet>
      )}
    </>
  );
};
