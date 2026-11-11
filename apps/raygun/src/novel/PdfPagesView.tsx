import { commands } from '@raygun/bindings';
import type { MangaChapterPages, NovelChapter, PdfChapter, PdfSearchHit } from '@raygun/bindings';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { FileText, ListTree, LoaderCircle, Search } from 'lucide-react';
import { useCallback, useMemo, useState } from 'react';
import type { FC, FormEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { PageReader } from '../manga/reader/PageReader';
import { Sheet } from '../shell/Sheet';
import { SourceIssue } from '../shell/SourceIssue';
import { SHEET_FIELD, SHEET_INPUT, TextField } from '../shell/TextField';
import { NOVEL, novelChapterQuery, saveNovelProgress } from './useNovel';

const buttonClass = 'hover:bg-background grid size-11 shrink-0 place-items-center rounded-full';

/** Where a search or bookmark leads: a page of this chapter or of another. */
export type PdfPlace = { chapterId: string; page: number };

/** Finds words in a PDF's text and lists where, page by page. */
const PdfSearchSheet: FC<{
  titleId: string;
  chapterId: string;
  onPick: (place: PdfPlace) => void;
  onClose: () => void;
}> = ({ titleId, chapterId, onPick, onClose }) => {
  const { t } = useTranslation();
  const [query, setQuery] = useState('');
  const search = useMutation({
    mutationFn: (words: string) => unwrapResult(commands.novelPdfSearch(titleId, chapterId, words)),
  });
  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (query.trim()) search.mutate(query.trim());
  };
  const hits: PdfSearchHit[] = search.data ?? [];
  return (
    <Sheet title={t('pdf.search')} onClose={onClose}>
      <form role="search" onSubmit={submit} className="flex flex-col gap-3">
        <TextField
          type="search"
          icon={Search}
          value={query}
          onChange={setQuery}
          aria-label={t('pdf.searchLabel')}
          placeholder={t('pdf.searchPlaceholder')}
          className={SHEET_FIELD}
          inputClassName={SHEET_INPUT}
          enterKeyHint="search"
          autoFocus
        />
      </form>
      {search.isPending && (
        <LoaderCircle
          aria-label={t('pdf.searching')}
          className="text-muted mx-auto my-6 animate-spin"
        />
      )}
      {search.error && <SourceIssue>{search.error.message}</SourceIssue>}
      {search.isSuccess && hits.length === 0 && (
        <p className="text-muted py-4 text-center text-sm">{t('pdf.noHits')}</p>
      )}
      {hits.length > 0 && (
        <ul aria-label={t('pdf.hits', { count: hits.length })} className="-mx-2 flex flex-col">
          {hits.map((hit, index) => (
            <li key={`${hit.chapterId}:${hit.page}:${index}`}>
              <button
                type="button"
                onClick={() => onPick({ chapterId: hit.chapterId, page: hit.page })}
                className="hover:bg-background flex w-full flex-col gap-0.5 rounded-xl px-3 py-2 text-left"
              >
                <span className="text-muted text-xs">
                  {t('pdf.page', { page: hit.pageNumber })}
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
    </Sheet>
  );
};

/** The document's bookmarks within the chapter, indented by level. */
const PdfContentsSheet: FC<{
  pdf: PdfChapter;
  onPick: (page: number) => void;
  onClose: () => void;
}> = ({ pdf, onPick, onClose }) => {
  const { t } = useTranslation();
  return (
    <Sheet title={t('pdf.contents')} onClose={onClose}>
      <ul className="-mx-2 flex flex-col">
        {pdf.contents.map((entry, index) => (
          <li key={`${entry.page}:${index}`}>
            <button
              type="button"
              onClick={() => onPick(entry.page)}
              style={{ paddingLeft: `${0.75 + Math.min(entry.level, 6) * 1}rem` }}
              className="hover:bg-background flex w-full items-baseline gap-3 rounded-xl py-2 pr-3 text-left"
            >
              <span className="min-w-0 flex-1 text-sm">{entry.title}</span>
              <span className="text-muted shrink-0 text-xs tabular-nums">
                {pdf.firstPage + entry.page}
              </span>
            </button>
          </li>
        ))}
      </ul>
    </Sheet>
  );
};

/**
 * A PDF chapter read as its pages, in the page reader (zoom, page turns, the AI layers), with its
 * text a tap away, a search through the document and its bookmarks. Progress is the page reached.
 */
export const PdfPagesView: FC<{
  data: NovelChapter;
  pdf: PdfChapter;
  onRetry: () => void;
  /** Opens another chapter, at a page when given. */
  onGo: (chapterId: string, page?: number) => void;
  onTextView: () => void;
  /** A page to show first (a search result opened from another chapter). */
  goTo?: { page: number; nonce: number } | null;
}> = ({ data, pdf, onRetry, onGo, onTextView, goTo }) => {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [searching, setSearching] = useState(false);
  const [contents, setContents] = useState(false);
  const [jump, setJump] = useState<{ page: number; nonce: number } | null>(goTo ?? null);
  const pages: MangaChapterPages = useMemo(
    () => ({
      titleId: data.titleId,
      title: data.title,
      readerMode: pdf.readerMode,
      ownReaderMode: pdf.ownReaderMode,
      chapter: data.chapter,
      pages: pdf.pages,
      previousChapterId: data.previousChapterId,
      nextChapterId: data.nextChapterId,
    }),
    [data, pdf],
  );
  const pagesOf = useCallback(
    (next: string) =>
      queryClient
        .fetchQuery(novelChapterQuery(data.titleId, next))
        .then((upcoming) => upcoming.pdf?.pages ?? []),
    [queryClient, data.titleId],
  );
  const onLeave = useCallback(() => {
    void queryClient.invalidateQueries({ queryKey: [NOVEL, 'title'] });
    void queryClient.invalidateQueries({ queryKey: [NOVEL, 'library'] });
  }, [queryClient]);
  const show = (page: number) => setJump((last) => ({ page, nonce: (last?.nonce ?? 0) + 1 }));
  const open = (place: PdfPlace) => {
    setSearching(false);
    if (place.chapterId === data.chapter.id) show(place.page);
    else onGo(place.chapterId, place.page);
  };

  return (
    <>
      <PageReader
        data={pages}
        error={null}
        onRetry={onRetry}
        medium="novel"
        onGo={(chapterId) => onGo(chapterId)}
        pagesOf={pagesOf}
        saveProgress={saveNovelProgress}
        onLeave={onLeave}
        goTo={jump}
        actions={
          <>
            <button
              type="button"
              aria-label={t('pdf.search')}
              aria-haspopup="dialog"
              onClick={() => setSearching(true)}
              className={buttonClass}
            >
              <Search aria-hidden size={21} />
            </button>
            {pdf.contents.length > 0 && (
              <button
                type="button"
                aria-label={t('pdf.contents')}
                aria-haspopup="dialog"
                onClick={() => setContents(true)}
                className={buttonClass}
              >
                <ListTree aria-hidden size={21} />
              </button>
            )}
            <button
              type="button"
              aria-label={t('pdf.textView')}
              onClick={onTextView}
              className={buttonClass}
            >
              <FileText aria-hidden size={21} />
            </button>
          </>
        }
      />
      {searching && (
        <PdfSearchSheet
          titleId={data.titleId}
          chapterId={data.chapter.id}
          onPick={open}
          onClose={() => setSearching(false)}
        />
      )}
      {contents && (
        <PdfContentsSheet
          pdf={pdf}
          onPick={(page) => {
            setContents(false);
            show(page);
          }}
          onClose={() => setContents(false)}
        />
      )}
    </>
  );
};
