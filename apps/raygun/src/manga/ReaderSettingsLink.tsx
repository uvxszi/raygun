import { Link } from '@tanstack/react-router';
import { ChevronRight, Settings } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

/** The last row of a reader's display sheet: Settings → Reader, with everything else. */
export const ReaderSettingsLink: FC = () => {
  const { t } = useTranslation();
  return (
    <Link
      to="/settings/reader"
      className="hover:bg-background flex items-center gap-3 rounded-xl px-2 py-3 text-sm"
    >
      <Settings aria-hidden size={20} className="text-muted shrink-0" />
      <span className="flex-1">{t('settings.allReaderSettings')}</span>
      <ChevronRight aria-hidden size={18} className="text-muted shrink-0" />
    </Link>
  );
};
