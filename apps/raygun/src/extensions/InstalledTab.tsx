import { Link } from '@tanstack/react-router';
import { ChevronRight } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useSettings } from '../settings/useSettings';
import { LetterAvatar } from '../shell/LetterAvatar';
import { useExtensions } from './useExtensions';

export const InstalledTab: FC = () => {
  const { t } = useTranslation();
  const extensions = useExtensions();
  const showNsfw = useSettings().data?.showNsfw ?? false;
  const error = extensions.error;

  return (
    <div data-testid="installed-tab" className="flex flex-col gap-3">
      {error && (
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {t('common.coreUnavailable', { message: error.message })}
        </p>
      )}
      {extensions.data?.length === 0 && (
        <p className="text-muted text-sm">{t('extensions.noneInstalled')}</p>
      )}
      <ul className="flex flex-col">
        {extensions.data?.map((extension) => (
          <li key={`${extension.origin}-${extension.id}`}>
            <Link
              to="/extensions/detail"
              search={{ id: extension.id }}
              data-testid="extension-row"
              className="hover:bg-surface flex items-center gap-4 rounded-xl px-1 py-2.5"
            >
              <LetterAvatar name={extension.name} />
              <span className="flex min-w-0 flex-1 flex-col gap-0.5">
                <span className="flex flex-wrap items-baseline gap-x-2">
                  <span className="font-medium">{extension.name}</span>
                  <span className="text-muted text-xs">{extension.version}</span>
                  <span className="bg-surface text-muted rounded px-1.5 text-[10px] font-semibold tracking-wide uppercase">
                    {t(
                      extension.origin === 'builtIn'
                        ? 'extensions.builtIn'
                        : 'extensions.installedOrigin',
                    )}
                  </span>
                  {extension.contentRating === 'nsfw' && (
                    <span className="bg-primary/20 text-primary rounded px-1.5 text-xs font-semibold">
                      {t('extensions.nsfwBadge')}
                    </span>
                  )}
                </span>
                {extension.contentRating === 'nsfw' && !showNsfw && (
                  <span className="text-muted text-xs">{t('extensions.nsfwHidden')}</span>
                )}
                {extension.status === 'failed' ? (
                  <span role="alert" className="text-primary truncate text-xs">
                    {t('extensions.failed', { message: extension.error ?? '' })}
                  </span>
                ) : (
                  extension.description && (
                    <span className="text-muted truncate text-xs">{extension.description}</span>
                  )
                )}
              </span>
              <ChevronRight aria-hidden size={18} className="text-muted shrink-0" />
            </Link>
          </li>
        ))}
      </ul>
    </div>
  );
};
