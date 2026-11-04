import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { SourceIssue } from '../shell/SourceIssue';

/**
 * Sources that couldn't search, quietly after the results: one line naming the source (or how
 * many), each one's error under More info.
 */
export const FailedSources: FC<{ failed: { name: string; error: string }[] }> = ({ failed }) => {
  const { t } = useTranslation();
  if (failed.length === 0) return null;
  return (
    <SourceIssue
      compact
      testId="failed-sources"
      source={
        failed.length === 1 ? failed[0]!.name : t('search.sourcesCount', { count: failed.length })
      }
    >
      {failed.map((source) => (
        <span key={source.name} className="block">
          {source.name}: {source.error}
        </span>
      ))}
    </SourceIssue>
  );
};
