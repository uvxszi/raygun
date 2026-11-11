import { commands } from '@raygun/bindings';
import type { BookSearch, BookSearchHit } from '@raygun/bindings';
import { useMutation } from '@tanstack/react-query';
import { LoaderCircle, Search } from 'lucide-react';
import { useEffect, useState } from 'react';
import type { FC, FormEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { Sheet } from '../shell/Sheet';
import { SourceIssue } from '../shell/SourceIssue';
import { SHEET_FIELD, SHEET_INPUT, TextField } from '../shell/TextField';

/** A place a search found: a paragraph of a chapter, or a PDF's page. */
export type BookPlace = { chapterId: string; block: number } | { chapterId: string; page: number };

/** A PDF's search answer, as the book search lists it. */
const pdfSearch = async (
  titleId: string,
  chapterId: string,
  words: string,
): Promise<BookSearch> => {
  const hits = await unwrapResult(commands.novelPdfSearch(titleId, chapterId, words));
  return {
    // The page's own number stands in for the chapter's name.
    hits: hits.map((hit) => ({
      chapterId: hit.chapterId,
      chapterName: String(hit.pageNumber),
      block: hit.page,
      before: hit.before,
      matched: hit.matched,
      after: hit.after,
    })),
    searched: 1,
    chapters: 1,
    more: false,
  };
};

/**
 * Where words appear in the book: every chapter on this device (an imported book's, a source's
 * downloaded ones and the one open), or a PDF's whole document. A place opens in the reader.
 */
export const BookSearchSheet: FC<{
  titleId: string;
  chapterId: string;
  /** The words to look for first. */
  initial: string;
  /** A PDF's text: places are its pages. */
  pdf: boolean;
  onPick: (place: BookPlace) => void;
  onClose: () => void;
}> = ({ titleId, chapterId, initial, pdf, onPick, onClose }) => {
  const { t } = useTranslation();
  const [query, setQuery] = useState(initial);
  const search = useMutation({
    mutationFn: (words: string) =>
      pdf
        ? pdfSearch(titleId, chapterId, words)
        : unwrapResult(commands.novelSearchBook(titleId, chapterId, words)),
  });
  const { mutate } = search;
  useEffect(() => {
    if (initial.trim()) mutate(initial.trim());
  }, [initial, mutate]);
  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (query.trim()) search.mutate(query.trim());
  };
  const result = search.data;
  const hits: BookSearchHit[] = result?.hits ?? [];
  return (
    <Sheet title={t('dictionary.search.title')} onClose={onClose}>
      <form role="search" onSubmit={submit} className="flex flex-col gap-3">
        <TextField
          type="search"
          icon={Search}
          value={query}
          onChange={setQuery}
          aria-label={t('dictionary.search.label')}
          placeholder={t('dictionary.search.placeholder')}
          className={SHEET_FIELD}
          inputClassName={SHEET_INPUT}
          enterKeyHint="search"
        />
      </form>
      {search.isPending && (
        <LoaderCircle
          aria-label={t('dictionary.search.searching')}
          className="text-muted mx-auto my-6 animate-spin"
        />
      )}
      {search.error && <SourceIssue>{search.error.message}</SourceIssue>}
      {result && !pdf && result.searched < result.chapters && (
        <p className="text-muted text-xs">
          {t('dictionary.search.partly', { searched: result.searched, chapters: result.chapters })}
        </p>
      )}
      {search.isSuccess && hits.length === 0 && (
        <p className="text-muted py-4 text-center text-sm">{t('dictionary.search.none')}</p>
      )}
      {hits.length > 0 && (
        <ul
          aria-label={t('dictionary.search.hits', { count: hits.length })}
          className="-mx-2 flex flex-col"
        >
          {hits.map((hit, index) => (
            <li key={`${hit.chapterId}:${hit.block}:${index}`}>
              <button
                type="button"
                onClick={() =>
                  onPick(
                    pdf
                      ? { chapterId: hit.chapterId, page: hit.block }
                      : { chapterId: hit.chapterId, block: hit.block },
                  )
                }
                className="hover:bg-background flex w-full flex-col gap-0.5 rounded-xl px-3 py-2 text-left"
              >
                <span className="text-muted text-xs">
                  {pdf ? t('pdf.page', { page: Number(hit.chapterName) }) : hit.chapterName}
                </span>
                <span className="text-sm">
                  {hit.before}
                  <mark className="bg-primary/25 text-foreground rounded px-0.5">
                    {hit.matched}
                  </mark>
                  {hit.after}
                </span>
              </button>
            </li>
          ))}
        </ul>
      )}
      {result?.more && (
        <p className="text-muted text-xs">{t('dictionary.search.more', { count: hits.length })}</p>
      )}
    </Sheet>
  );
};
