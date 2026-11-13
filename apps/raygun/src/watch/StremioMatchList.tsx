import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { MangaCover } from '../manga/MangaCover';
import { MatchRow } from '../manga/MatchRow';
import type { StremioMatch } from './stremioMatches';

/**
 * The Stremio addons' matches as Find in other sources lists them (`stremioMatches`), each
 * opening its title's page there, where every addon's streams come for its id.
 */
export const StremioMatchList: FC<{ matches: StremioMatch[] }> = ({ matches }) => {
  const { t } = useTranslation();
  if (matches.length === 0) return null;
  return (
    <section aria-label={t('otherSources.stremioAddons')} className="flex flex-col">
      <h3 className="text-muted text-xs font-semibold">{t('otherSources.stremioAddons')}</h3>
      <ul className="flex flex-col">
        {matches.map(({ item, addonName }) => (
          <li key={`${item.kind}:${item.id}`}>
            <MatchRow
              open={{ to: '/watch/title', search: { type: item.kind, id: item.id } }}
              cover={<MangaCover url={item.poster} video className="w-10 rounded" />}
              title={item.name}
              lines={[item.releaseInfo, addonName]}
            />
          </li>
        ))}
      </ul>
    </section>
  );
};
