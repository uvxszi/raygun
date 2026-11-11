import type { VideoLinkFile, VideoLinkMatch, VideoLinkPage as LinkView } from '@raygun/bindings';
import { Link, useCanGoBack, useRouter, useSearch } from '@tanstack/react-router';
import {
  Captions,
  Copy,
  Download,
  FileVideo,
  FolderOpen,
  Image,
  LoaderCircle,
  Pencil,
  Play,
  RotateCcw,
  SearchCheck,
  Trash2,
  File as FileIcon,
} from 'lucide-react';
import { memo, useCallback, useState } from 'react';
import type { FC, FormEvent, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { formatBytes } from '../lib/formatBytes';
import { OtherSources } from '../manga/OtherSources';
import { buildMenu, copyText } from '../music/mediaMenu';
import { MenuButton } from '../shell/MenuButton';
import { PinButton } from '../shell/PinButton';
import { Sheet } from '../shell/Sheet';
import { SourceIssue } from '../shell/SourceIssue';
import { SHEET_FIELD, SHEET_INPUT, TextField } from '../shell/TextField';
import { ResumeFab, TitleHero } from '../shell/TitleHero';
import { showToast } from '../shell/toastStore';
import { TopBar, TopBarAction } from '../shell/TopBar';
import { useWideScreen } from '../shell/useWideScreen';
import { VirtualRows } from '../shell/VirtualRows';
import { rowsEqual } from '../shell/windowedRows';
import type { RowProps } from '../shell/windowedRows';
import { StremioPoster } from '../watch/StremioRow';
import { VideoDownloadControl } from '../watch/VideoDownloadControl';
import { useStremioSearchRows } from '../watch/useStremio';
import { useDownloadsByKey, videoTitleLink, STREMIO_SOURCE } from '../watch/useVideoLibrary';
import {
  useDownloadVideoLink,
  useMatchVideoLink,
  usePlayVideoLink,
  useRenameVideoLink,
  useVideoLinkFiles,
  useVideoLinkPage,
  useVideoLinkSuggestions,
} from '../watch/useVideoLink';
import { useRemoveContinue } from '../watch/useWatch';
import { videoTitleScore } from '../watch/videoMatch';

/** A found title is at least this close to the link's (search's own scale, 100 = the same). */
const CLOSE_ENOUGH = 60;

const FILE_ICONS = {
  video: FileVideo,
  subtitle: Captions,
  image: Image,
  other: FileIcon,
} as const;

/** Started and not finished: resuming means something. */
const started = (view: LinkView) => !view.watched && (view.positionMs ?? 0) > 1;

const Progress: FC<{ view: LinkView }> = ({ view }) => {
  const duration = view.durationMs ?? 0;
  return started(view) && duration > 0 ? (
    <span aria-hidden className="block h-1 overflow-hidden rounded-full bg-black/20">
      <span
        className="bg-primary block h-full"
        style={{ width: `${Math.min(100, ((view.positionMs ?? 0) / duration) * 100)}%` }}
      />
    </span>
  ) : null;
};

/** Renames the link: its page, Continue watching and the player show the new name. */
const RenameSheet: FC<{ view: LinkView; onClose: () => void }> = ({ view, onClose }) => {
  const { t } = useTranslation();
  const rename = useRenameVideoLink();
  const [title, setTitle] = useState(view.title);
  const onSubmit = (event: FormEvent) => {
    event.preventDefault();
    if (!title.trim()) return;
    rename.mutate({ key: view.key, title: title.trim() }, { onSuccess: onClose });
  };
  return (
    <Sheet title={t('watch.link.renameTitle')} onClose={onClose}>
      <form onSubmit={onSubmit} className="flex flex-col gap-3">
        <TextField
          autoFocus
          value={title}
          onChange={setTitle}
          aria-label={t('watch.link.name')}
          className={SHEET_FIELD}
          inputClassName={`${SHEET_INPUT} text-sm`}
        />
        {rename.error && <SourceIssue>{rename.error.message}</SourceIssue>}
        <button
          type="submit"
          disabled={!title.trim() || rename.isPending}
          className="bg-primary text-background self-end rounded-full px-5 py-2.5 text-sm font-semibold disabled:opacity-40"
        >
          {t('watch.link.save')}
        </button>
      </form>
    </Sheet>
  );
};

/**
 * One file of a torrent: a video plays or downloads on its own; a subtitle file says its
 * language. Everything it shows comes as props (a torrent may hold hundreds of files).
 */
const FileRow = memo(
  ({
    file,
    canDownload,
    onPlay,
    onDownload,
    row,
  }: {
    file: VideoLinkFile;
    canDownload: boolean;
    onPlay: (file: VideoLinkFile) => void;
    onDownload: (file: VideoLinkFile) => void;
    row: RowProps;
  }) => {
    const { t } = useTranslation();
    const Icon = FILE_ICONS[file.kind];
    const video = file.kind === 'video';
    const kindLabel =
      file.kind === 'subtitle'
        ? file.language
          ? t('watch.link.subtitlesIn', { language: file.language })
          : t('watch.link.subtitles')
        : file.kind === 'image'
          ? t('watch.link.picture')
          : null;
    return (
      <li
        {...row}
        aria-label={file.name}
        className="flex min-h-12 items-center gap-3 border-b border-white/5 py-1.5"
      >
        <Icon aria-hidden size={18} className={video ? 'text-primary' : 'text-muted'} />
        <span className="flex min-w-0 flex-1 flex-col">
          <span className={`truncate text-sm ${video ? '' : 'text-muted'}`}>{file.name}</span>
          <span className="text-muted text-xs">
            {[
              formatBytes(file.sizeBytes ?? 0),
              file.main ? t('watch.link.opensFirst') : null,
              kindLabel,
              file.path.includes('/') ? file.path.slice(0, file.path.lastIndexOf('/')) : null,
            ]
              .filter(Boolean)
              .join(' · ')}
          </span>
        </span>
        {video && (
          <>
            <button
              type="button"
              aria-label={t('watch.link.playFile', { name: file.name })}
              onClick={() => onPlay(file)}
              className="hover:bg-surface grid size-9 place-items-center rounded-full"
            >
              <Play aria-hidden size={16} fill="currentColor" />
            </button>
            {canDownload && (
              <button
                type="button"
                aria-label={t('watch.link.downloadFile', { name: file.name })}
                onClick={() => onDownload(file)}
                className="text-muted hover:bg-surface grid size-9 place-items-center rounded-full"
              >
                <Download aria-hidden size={16} />
              </button>
            )}
          </>
        )}
      </li>
    );
  },
  rowsEqual,
);

/**
 * A torrent's files: its videos play (the one the link opens first) or download on their own,
 * its subtitle files say their language. Until they're known (it opened, or peers were asked),
 * a button asks peers for them.
 */
const LinkFiles: FC<{ view: LinkView }> = ({ view }) => {
  const { t } = useTranslation();
  const listFiles = useVideoLinkFiles();
  const play = usePlayVideoLink();
  const download = useDownloadVideoLink();
  const files = view.files;
  const key = view.key;
  const playMutate = play.mutate;
  const downloadMutate = download.mutate;
  const onPlay = useCallback(
    (file: VideoLinkFile) => playMutate({ key, file: file.index }),
    [playMutate, key],
  );
  const onDownload = useCallback(
    (file: VideoLinkFile) =>
      downloadMutate(
        { key, file: file.index },
        { onSuccess: () => showToast(t('watch.link.downloadQueued', { name: file.name })) },
      ),
    [downloadMutate, key, t],
  );
  return (
    <section aria-labelledby="link-files" className="flex flex-col gap-2">
      <h2 id="link-files" className="font-semibold">
        {t('watch.link.files')}{' '}
        {files && (
          <span className="text-muted ml-1 text-sm font-normal">
            {t('watch.link.filesCount', { count: files.length })}
          </span>
        )}
      </h2>
      {!files && (
        <div className="flex flex-col items-start gap-2">
          <p className="text-muted text-sm">{t('watch.link.filesUnknown')}</p>
          <button
            type="button"
            disabled={listFiles.isPending}
            onClick={() => listFiles.mutate(view.key)}
            className="bg-surface flex items-center gap-2 rounded-full px-4 py-2 text-sm disabled:opacity-60"
          >
            {listFiles.isPending ? (
              <LoaderCircle aria-hidden size={16} className="animate-spin" />
            ) : (
              <FolderOpen aria-hidden size={16} />
            )}
            {listFiles.isPending ? t('watch.link.findingFiles') : t('watch.link.showFiles')}
          </button>
          {listFiles.error && <SourceIssue>{listFiles.error.message}</SourceIssue>}
        </div>
      )}
      {files && (
        <VirtualRows
          items={files}
          keyOf={(file) => String(file.index)}
          rowRem={3.25}
          className="flex flex-col"
          renderRow={(file, _index, row) => (
            <FileRow
              file={file}
              canDownload={view.canDownload}
              onPlay={onPlay}
              onDownload={onDownload}
              row={row}
            />
          )}
        />
      )}
      {(play.error ?? download.error) && (
        <SourceIssue>{(play.error ?? download.error)?.message}</SourceIssue>
      )}
    </section>
  );
};

/** A Stremio title's page. */
const stremioPage = (kind: string, id: string) => videoTitleLink(STREMIO_SOURCE, `${kind}:${id}`);

/** One movie or series a link may be, as a row: its poster, name and year, and what to do. */
const MatchRow: FC<{ match: VideoLinkMatch; children: ReactNode }> = ({ match, children }) => (
  <li aria-label={match.name} className="flex items-center gap-3 py-1.5">
    <StremioPoster url={match.poster} className="w-10 shrink-0 rounded-md" />
    <span className="flex min-w-0 flex-1 flex-col">
      <span className="truncate text-sm font-medium">{match.name}</span>
      <span className="text-muted text-xs">{match.year}</span>
    </span>
    {children}
  </li>
);

/**
 * What the link is: the movie or series it was matched with (its poster shows for the link, and
 * a movie's other sources join the player's), or, once asked, the ones its title may be.
 */
const LinkMatchSection: FC<{ view: LinkView }> = ({ view }) => {
  const { t } = useTranslation();
  const [asked, setAsked] = useState(false);
  const suggestions = useVideoLinkSuggestions(view.key, asked && !view.matched);
  const match = useMatchVideoLink();
  const matched = view.matched;
  return (
    <section
      aria-labelledby="link-match"
      className="bg-surface flex flex-col gap-2 rounded-2xl p-3"
    >
      <h2 id="link-match" className="text-sm font-semibold">
        {t('watch.link.matchTitle')}
      </h2>
      {matched ? (
        <ul>
          <MatchRow match={matched}>
            <Link
              to={stremioPage(matched.kind, matched.id).to}
              search={stremioPage(matched.kind, matched.id).search as never}
              className="bg-background rounded-full px-3 py-1.5 text-xs font-semibold"
            >
              {t('watch.link.openMatch')}
            </Link>
            <button
              type="button"
              onClick={() => match.mutate({ key: view.key, matched: null })}
              className="text-muted rounded-full px-3 py-1.5 text-xs"
            >
              {t('watch.link.unmatch')}
            </button>
          </MatchRow>
        </ul>
      ) : asked ? (
        <>
          {suggestions.isPending && (
            <LoaderCircle
              aria-label={t('watch.loading')}
              size={18}
              className="text-muted animate-spin"
            />
          )}
          {suggestions.error && <SourceIssue>{suggestions.error.message}</SourceIssue>}
          {suggestions.data?.length === 0 && (
            <p className="text-muted text-sm">
              {t('watch.link.noMatches', { query: view.searchTitle })}
            </p>
          )}
          <ul>
            {(suggestions.data ?? []).map((found) => (
              <MatchRow key={`${found.kind}:${found.id}`} match={found}>
                <button
                  type="button"
                  disabled={match.isPending}
                  aria-label={t('watch.link.useMatch', { name: found.name })}
                  onClick={() => match.mutate({ key: view.key, matched: found })}
                  className="bg-primary text-background rounded-full px-3 py-1.5 text-xs font-semibold disabled:opacity-60"
                >
                  {t('watch.link.thisIsIt')}
                </button>
              </MatchRow>
            ))}
          </ul>
        </>
      ) : (
        <div className="flex flex-col items-start gap-2">
          <p className="text-muted text-sm">{t('watch.link.matchHint')}</p>
          <button
            type="button"
            onClick={() => setAsked(true)}
            className="bg-background flex items-center gap-2 rounded-full px-4 py-2 text-sm"
          >
            <SearchCheck aria-hidden size={16} />
            {t('watch.link.findMatches')}
          </button>
        </div>
      )}
      {match.error && <SourceIssue>{match.error.message}</SourceIssue>}
    </section>
  );
};

/** Movies and series the installed addons find under the link's title, to open their pages. */
const AddonMatches: FC<{ title: string }> = ({ title }) => {
  const { t } = useTranslation();
  const search = useStremioSearchRows(title);
  const found = search.rows
    .flatMap((row) => row.items.map((item) => ({ item, addon: row.addonName })))
    .filter(({ item }) => videoTitleScore(item.name, title, false) >= CLOSE_ENOUGH)
    .filter(
      ({ item }, index, all) =>
        all.findIndex((other) => other.item.kind === item.kind && other.item.id === item.id) ===
        index,
    );
  if (!search.pending && found.length === 0) return null;
  return (
    <section
      aria-label={t('watch.link.addonMatches')}
      className="bg-surface flex flex-col gap-2 rounded-2xl p-3"
    >
      <h2 className="flex items-center gap-2 text-sm font-semibold">
        {t('watch.link.addonMatches')}
        {search.pending && (
          <LoaderCircle aria-hidden size={14} className="text-muted animate-spin" />
        )}
      </h2>
      <ul>
        {found.map(({ item, addon }) => (
          <li key={`${item.kind}:${item.id}`}>
            <Link
              to={stremioPage(item.kind, item.id).to}
              search={stremioPage(item.kind, item.id).search as never}
              className="hover:bg-background flex items-center gap-3 rounded-xl py-1.5"
            >
              <StremioPoster url={item.poster} className="w-10 shrink-0 rounded-md" />
              <span className="flex min-w-0 flex-col">
                <span className="truncate text-sm font-medium">{item.name}</span>
                <span className="text-muted text-xs">
                  {[item.releaseInfo, addon].filter(Boolean).join(' · ')}
                </span>
              </span>
            </Link>
          </li>
        ))}
      </ul>
    </section>
  );
};

/**
 * A link played on its own (Open link, another app): what it is (a torrent's name, size and
 * files; a web address's host and format), its name (renamed here), its poster (a torrent's own,
 * or the movie it's matched with), Play or Resume, Download, Remove, and the same title in other
 * sources.
 */
export const VideoLinkPage: FC = () => {
  const { t } = useTranslation();
  const wide = useWideScreen();
  const router = useRouter();
  const canGoBack = useCanGoBack();
  const { key } = useSearch({ from: '/watch/link' });
  const page = useVideoLinkPage(key);
  const play = usePlayVideoLink();
  const download = useDownloadVideoLink();
  const downloads = useDownloadsByKey();
  const remove = useRemoveContinue();
  const [renaming, setRenaming] = useState(false);
  const [finding, setFinding] = useState(false);
  const view = page.data;

  const leave = () =>
    canGoBack
      ? router.history.back()
      : void router.navigate({ to: '/library', search: { media: 'video' }, replace: true });

  if (!view) {
    return (
      <section data-testid="video-link-page" className="flex flex-col gap-5">
        <TopBar back fallback="/library" title={t('watch.link.page')} />
        {page.error ? (
          <SourceIssue>{page.error.message}</SourceIssue>
        ) : (
          <LoaderCircle
            aria-label={t('watch.loading')}
            className="text-muted mx-auto my-12 animate-spin"
          />
        )}
      </section>
    );
  }

  const resumes = started(view);
  const playLabel = t(resumes ? 'watch.resume' : 'watch.play');
  const menu = () =>
    buildMenu({
      heading: { label: view.title, subtitle: view.host, artworkUrl: view.poster },
      play: [
        resumes && {
          label: t('watch.continueMenu.fromStart'),
          icon: RotateCcw,
          onSelect: () => play.mutate({ key: view.key, fromStart: true }),
        },
      ],
      share: [
        {
          label: t('watch.link.copyAddress'),
          icon: Copy,
          onSelect: () => {
            copyText(view.url);
            showToast(t('watch.link.copied'));
          },
        },
      ],
      edit: [{ label: t('watch.link.rename'), icon: Pencil, onSelect: () => setRenaming(true) }],
      remove: [
        {
          label: t('watch.link.remove'),
          icon: Trash2,
          confirm: t('watch.link.removeAsk', { title: view.title }),
          onSelect: () =>
            remove.mutate(view.key, {
              onSuccess: () => {
                showToast(t('watch.link.removed', { title: view.title }));
                leave();
              },
              onError: (error) => showToast(error.message),
            }),
        },
      ],
    });

  return (
    <section data-testid="video-link-page" className="flex min-h-full flex-col gap-5">
      <TopBar
        back
        fallback="/library"
        plainTitle
        title={view.host ?? t('watch.link.page')}
        actions={
          <>
            <PinButton
              target={{
                kind: 'video',
                to: '/watch/link',
                search: { key: view.key },
                name: view.title,
                artworkUrl: view.poster,
              }}
            />
            <TopBarAction
              label={t('watch.link.rename')}
              icon={Pencil}
              onClick={() => setRenaming(true)}
            />
            <MenuButton label={t('menu.options', { name: view.title })} items={menu} />
          </>
        }
      />
      <TitleHero
        art={view.poster}
        cover={<StremioPoster url={view.poster} className="w-full shadow-lg" />}
        title={view.title}
        facts={[
          t(`watch.link.kinds.${view.kind}`),
          view.format,
          view.sizeBytes !== null ? formatBytes(view.sizeBytes) : null,
          view.host,
          view.watched && t('watch.watched'),
        ]}
        source={
          view.matched
            ? `${view.matched.name}${view.matched.year ? ` (${view.matched.year})` : ''}`
            : null
        }
      >
        <div className="flex flex-col gap-3">
          <div className="flex flex-wrap items-center gap-2">
            {wide && (
              <button
                type="button"
                onClick={() => play.mutate({ key: view.key })}
                className="bg-primary text-background flex items-center gap-2 rounded-full px-5 py-2 text-sm font-semibold"
              >
                <Play aria-hidden size={16} fill="currentColor" />
                {playLabel}
              </button>
            )}
            {resumes && (
              <button
                type="button"
                onClick={() => play.mutate({ key: view.key, fromStart: true })}
                className="bg-surface/80 flex items-center gap-2 rounded-full px-4 py-2 text-sm"
              >
                <RotateCcw aria-hidden size={16} />
                {t('watch.continueMenu.fromStart')}
              </button>
            )}
            {view.canDownload && (
              <span className="bg-surface/80 rounded-full">
                <VideoDownloadControl
                  item={downloads.get(view.key)}
                  downloaded={false}
                  name={view.title}
                  onDownload={() =>
                    download.mutate(
                      { key: view.key },
                      {
                        onSuccess: () =>
                          showToast(t('watch.link.downloadQueued', { name: view.title })),
                      },
                    )
                  }
                />
              </span>
            )}
          </div>
          <Progress view={view} />
        </div>
      </TitleHero>
      {(play.error ?? download.error) && (
        <SourceIssue>{(play.error ?? download.error)?.message}</SourceIssue>
      )}
      {view.kind === 'torrent' && <LinkFiles view={view} />}
      <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-sm">
        {/* A torrent's own name, which a new name for the link doesn't change. */}
        {view.name && (
          <>
            <dt className="text-muted">{t('watch.link.torrentName')}</dt>
            <dd className="break-all">{view.name}</dd>
          </>
        )}
        <dt className="text-muted">{t('watch.link.address')}</dt>
        <dd className="line-clamp-3 break-all">{view.url}</dd>
      </dl>
      <LinkMatchSection view={view} />
      <button
        type="button"
        aria-expanded={finding}
        onClick={() => setFinding(!finding)}
        className="bg-surface hover:bg-primary/15 flex items-center gap-2 self-start rounded-full px-4 py-2 text-sm font-medium"
      >
        <SearchCheck aria-hidden size={16} />
        {t('otherSources.find')}
      </button>
      {finding && (
        <>
          <AddonMatches title={view.searchTitle} />
          <OtherSources medium="video" title={view.searchTitle} sourceId="" />
        </>
      )}
      {!wide && <ResumeFab label={playLabel} onClick={() => play.mutate({ key: view.key })} />}
      {renaming && <RenameSheet view={view} onClose={() => setRenaming(false)} />}
    </section>
  );
};
