import type { UpdateFailure } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { SourceIssue } from '../shell/SourceIssue';
import { videoTitleLink } from '../watch/useVideoLibrary';
import { ROUTES } from './medium';

const titleLink = (failure: UpdateFailure) =>
  failure.medium === 'video'
    ? videoTitleLink(failure.sourceId, failure.remoteId)
    : {
        to: failure.medium === 'novel' ? ROUTES.novel.title : ROUTES.manga.title,
        search: { source: failure.sourceId, id: failure.remoteId },
      };

/** The titles the last library update couldn't check, each with its source and error. */
export const UpdateFailures: FC<{ failures: UpdateFailure[] }> = ({ failures }) => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  if (failures.length === 0) return null;
  return (
    <div className="flex flex-col gap-2">
      <button
        type="button"
        aria-expanded={open}
        onClick={() => setOpen(!open)}
        className="text-primary self-start text-sm font-medium"
      >
        {t(open ? 'updates.hideFailures' : 'updates.showFailures')}
      </button>
      {open && (
        <ul data-testid="update-failures" className="flex flex-col gap-2">
          {failures.map((failure) => (
            <li key={`${failure.sourceId}:${failure.remoteId}`} className="flex flex-col gap-1">
              <Link {...titleLink(failure)} className="truncate text-sm font-medium">
                {failure.title}
              </Link>
              <SourceIssue compact source={failure.sourceName}>
                {failure.message}
              </SourceIssue>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
};
