import { Fragment } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { siteLabels } from '../extensions/siteLabels';
import { PrefBlock, PrefGroup, PrefInfo, PrefLink, PrefSwitch } from '../settings/prefs';
import { SourceIssue } from '../shell/SourceIssue';
import { useSetSiteScripts, useSiteScripts } from './useWebsites';

/**
 * Settings → Browse → Websites: the installed site scripts (`site.scripts` extensions), where
 * each runs, and a switch for each of the user's websites it runs on. A switch applies the next
 * time the site opens; the browser's menu has the same switch.
 */
export const SiteScriptsSettings: FC = () => {
  const { t } = useTranslation();
  const scripts = useSiteScripts();
  const setOn = useSetSiteScripts();
  const list = scripts.data ?? [];
  return (
    <PrefGroup title={t('websites.scriptsTitle')} summary={t('websites.scriptsHint')}>
      {scripts.isError && (
        <PrefBlock>
          <SourceIssue compact title={t('websites.scriptsLoadFailed')}>
            {scripts.error.message}
          </SourceIssue>
        </PrefBlock>
      )}
      {scripts.isSuccess && list.length === 0 && <PrefInfo title={t('websites.scriptsNone')} />}
      {list.map((extension) => (
        <Fragment key={extension.extensionId}>
          <PrefInfo
            title={extension.name}
            summary={t('websites.scriptsRunsOn', {
              sites: siteLabels(extension.sites, t).join(', '),
            })}
          />
          {extension.websites.length === 0 ? (
            <PrefInfo title={t('websites.scriptsNoWebsites')} />
          ) : (
            extension.websites.map((site) => (
              <PrefSwitch
                key={site.id}
                title={site.name}
                summary={t('websites.scriptsOnSite', { name: extension.name })}
                checked={site.on}
                disabled={setOn.isPending}
                onChange={(on) =>
                  setOn.mutate({ siteId: site.id, extensionId: extension.extensionId, on })
                }
              />
            ))
          )}
        </Fragment>
      ))}
      {setOn.isError && (
        <PrefBlock>
          <SourceIssue compact title={t('websites.saveFailed')}>
            {setOn.error.message}
          </SourceIssue>
        </PrefBlock>
      )}
      <PrefLink title={t('websites.scriptsFind')} to="/sources" />
    </PrefGroup>
  );
};
