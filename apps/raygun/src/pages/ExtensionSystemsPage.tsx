import { Link } from '@tanstack/react-router';
import {
  BookAudio,
  Clapperboard,
  FileUp,
  Globe,
  Music,
  Puzzle,
  ScrollText,
  Sparkles,
  Tv,
} from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { TopBar } from '../shell/TopBar';

type System = {
  id: string;
  icon: LucideIcon;
  /** Where to manage it. */
  to?: string;
};

/** Every kind of add-on Raygun runs or reads, in the order most people meet them. */
const SYSTEMS: System[] = [
  { id: 'raygun', icon: Puzzle, to: '/extensions/repos' },
  { id: 'nuclear', icon: Music, to: '/extensions/repos' },
  { id: 'shosetsu', icon: ScrollText, to: '/extensions/repos' },
  { id: 'miru', icon: Tv, to: '/extensions/repos' },
  { id: 'stremio', icon: Clapperboard, to: '/watch/addons' },
  { id: 'feeds', icon: BookAudio },
  { id: 'websites', icon: Globe },
  { id: 'imports', icon: FileUp },
  { id: 'others', icon: Sparkles },
];

/**
 * What Raygun can run (M46.23): its own extensions and every other system it loads or reads, what
 * each brings, and where to manage it. Also says what other players' add-ons became.
 */
export const ExtensionSystemsPage: FC = () => {
  const { t } = useTranslation();
  return (
    <section data-testid="extension-systems" className="flex flex-col gap-4">
      <TopBar back fallback="/browse" title={t('systems.title')} />
      <p className="text-muted text-sm">{t('systems.intro')}</p>
      <ul className="flex flex-col gap-3">
        {SYSTEMS.map(({ id, icon: Icon, to }) => (
          <li
            key={id}
            aria-label={t(`systems.items.${id}.name`)}
            className="bg-surface flex gap-4 rounded-2xl px-4 py-3"
          >
            <Icon aria-hidden size={22} className="text-primary mt-0.5 shrink-0" />
            <div className="flex min-w-0 flex-col gap-1">
              <h2 className="font-semibold">{t(`systems.items.${id}.name`)}</h2>
              <p className="text-sm">{t(`systems.items.${id}.what`)}</p>
              <p className="text-muted text-xs">{t(`systems.items.${id}.how`)}</p>
              {to && (
                <Link to={to} className="text-primary self-start text-sm font-semibold">
                  {t('systems.manage')}
                </Link>
              )}
            </div>
          </li>
        ))}
      </ul>
    </section>
  );
};
