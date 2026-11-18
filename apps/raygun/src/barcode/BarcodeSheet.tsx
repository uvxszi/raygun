import { commands } from '@raygun/bindings';
import type { BarcodeFind } from '@raygun/bindings';
import { useQueries } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { Camera, ImageUp, LoaderCircle, ScanBarcode } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import type { FC, FormEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { useQrScanner } from '../devices/qrScanner';
import { TAP_BUTTON } from '../devices/tapTargets';
import { unwrapResult } from '../lib/commandResult';
import { usePlatform } from '../lib/usePlatform';
import { canAsk, usePermissions, useRequestPermission } from '../permissions/usePermissions';
import { searchLink } from '../search/searchTypes';
import { Sheet } from '../shell/Sheet';
import { SourceIssue } from '../shell/SourceIssue';
import { SHEET_FIELD, SHEET_INPUT, TextField } from '../shell/TextField';
import { COLLECT_MS, ENOUGH_READS, countRead, isDominant, rankCandidates } from './candidates';
import type { Candidate, Tally } from './candidates';
import { readPicture } from './readPicture';

/** A code typed or scanned: digits only, as long as an EAN-8, UPC-A or EAN-13. */
const asCode = (text: string) => {
  const digits = text.replace(/[\s-]/g, '');
  return /^(\d{8}|\d{12}|\d{13})$/.test(digits) ? digits : null;
};

/** What a scanned code turned out to be, with where to go from it. */
const Found: FC<{ find: BarcodeFind; onDone: () => void; onAgain: () => void }> = ({
  find,
  onDone,
  onAgain,
}) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const go = (to: Parameters<typeof navigate>[0]) => {
    onDone();
    void navigate(to);
  };
  const again = (
    <button type="button" onClick={onAgain} className={`${TAP_BUTTON} bg-background`}>
      <ScanBarcode aria-hidden size={16} />
      {t('barcode.scanAgain')}
    </button>
  );
  if (find.kind === 'album') {
    const best = find.matches[0];
    const words = [find.artist, find.title].filter(Boolean).join(' ');
    return (
      <div className="flex flex-col gap-3">
        <p className="font-medium">
          {find.artist
            ? t('barcode.album', { title: find.title, artist: find.artist })
            : find.title}
          {find.year ? ` (${find.year})` : ''}
        </p>
        <div className="flex flex-wrap gap-2">
          {best ? (
            <button
              type="button"
              onClick={() =>
                go({
                  to: '/music/album',
                  search: { provider: best.provider.id, id: best.id },
                } as never)
              }
              className={`${TAP_BUTTON} bg-primary text-background font-semibold`}
            >
              {t('barcode.openAlbum')}
            </button>
          ) : null}
          <button
            type="button"
            onClick={() => go(searchLink('music', 'albums', words) as never)}
            className={`${TAP_BUTTON} bg-background`}
          >
            {t('barcode.searchAlbum')}
          </button>
          {again}
        </div>
      </div>
    );
  }
  if (find.kind === 'book') {
    return (
      <div className="flex flex-col gap-3">
        <div className="flex items-center gap-3">
          {find.coverUrl && (
            <img src={find.coverUrl} alt="" className="h-20 w-14 shrink-0 rounded object-cover" />
          )}
          <p className="flex flex-col">
            <span className="font-medium">{find.title}</span>
            <span className="text-muted text-sm">{find.authors.join(', ')}</span>
          </p>
        </div>
        <div className="flex flex-wrap gap-2">
          <button
            type="button"
            onClick={() => go(searchLink('novel', undefined, find.title) as never)}
            className={`${TAP_BUTTON} bg-primary text-background font-semibold`}
          >
            {t('barcode.findBook')}
          </button>
          {again}
        </div>
      </div>
    );
  }
  return (
    <div className="flex flex-col gap-3">
      <p>{t('barcode.notFound', { code: find.code })}</p>
      <div className="flex flex-wrap gap-2">
        <button
          type="button"
          onClick={() => go(searchLink('all', undefined, find.code) as never)}
          className={`${TAP_BUTTON} bg-background`}
        >
          {t('barcode.searchNumber', { code: find.code })}
        </button>
        {again}
      </div>
    </div>
  );
};

/** A code's cover and name as its lookup found them, for the list of codes read. */
const findSummary = (find: BarcodeFind | undefined) => {
  if (!find || find.kind === 'unknown') return null;
  if (find.kind === 'book') {
    return { cover: find.coverUrl, title: find.title, by: find.authors.join(', ') };
  }
  return { cover: find.matches[0]?.artworkUrl ?? null, title: find.title, by: find.artist ?? '' };
};

type Lookup = { data?: BarcodeFind; isPending: boolean; error: Error | null };

/** One code read, with what it finds; picking it shows what to do with it. */
const CandidateRow: FC<{ candidate: Candidate; lookup: Lookup; onPick: () => void }> = ({
  candidate,
  lookup,
  onPick,
}) => {
  const { t } = useTranslation();
  const summary = findSummary(lookup.data);
  return (
    <li>
      <button
        type="button"
        onClick={onPick}
        className="bg-background hover:bg-surface flex w-full items-center gap-3 rounded-xl p-2 text-left"
      >
        <span className="bg-surface grid h-16 w-12 shrink-0 place-items-center overflow-hidden rounded">
          {summary?.cover ? (
            <img src={summary.cover} alt="" className="size-full object-cover" />
          ) : lookup.isPending ? (
            <LoaderCircle aria-hidden size={18} className="text-muted animate-spin" />
          ) : (
            <ScanBarcode aria-hidden size={18} className="text-muted" />
          )}
        </span>
        <span className="flex min-w-0 flex-1 flex-col">
          <span className="truncate font-medium">
            {summary
              ? summary.title
              : lookup.isPending
                ? t('barcode.lookingShort')
                : t('barcode.unknownShort')}
          </span>
          {summary?.by ? <span className="text-muted truncate">{summary.by}</span> : null}
          <span className="text-muted text-xs tabular-nums">
            {t('barcode.reads', { code: candidate.code, count: candidate.reads })}
          </span>
        </span>
      </button>
    </li>
  );
};

/**
 * Search's scan button (v2.0.0): a CD's or record's barcode opens its album (MusicBrainz finds
 * the release, the music sources the album), a book's ISBN finds the book (Open Library, then
 * the book sources). The camera reads the code (Android, or a desktop webcam), or a picture of
 * it, or the number typed. The camera keeps reading for a moment after the first code (v2.0.1,
 * barcode/candidates.ts): the codes read most are looked up and offered, the clear winner first.
 */
export const BarcodeSheet: FC<{ onClose: () => void }> = ({ onClose }) => {
  const { t } = useTranslation();
  const platform = usePlatform();
  const android = platform === 'android';
  const permissions = usePermissions();
  const requestPermission = useRequestPermission();
  const video = useRef<HTMLVideoElement>(null);
  const picture = useRef<HTMLInputElement>(null);
  const [typed, setTyped] = useState('');
  const [noCode, setNoCode] = useState(false);
  // The codes read, once reading ended; the one picked from them.
  const [pick, setPick] = useState<Candidate[] | null>(null);
  const [chosen, setChosen] = useState<string | null>(null);
  // Reads while the camera collects them.
  const tally = useRef<Tally>(new Map());
  const collectTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const [collecting, setCollecting] = useState(false);
  const lookups: Lookup[] = useQueries({
    queries: (pick ?? []).map((candidate) => ({
      queryKey: ['barcodeLookup', candidate.code],
      queryFn: () => unwrapResult(commands.barcodeLookup(candidate.code)),
      staleTime: Infinity,
      retry: false,
    })),
  });
  const camera = permissions.data?.find((permission) => permission.kind === 'camera');
  const cameraAllowed =
    platform !== undefined &&
    (!android || (permissions.data !== undefined && (!camera || camera.state === 'granted')));
  useEffect(() => {
    if (android && camera && canAsk(camera) && !requestPermission.isPending) {
      requestPermission.mutate('camera');
    }
    // Asked once when the sheet opens.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [android, camera?.state]);
  useEffect(() => () => clearTimeout(collectTimer.current), []);

  const finish = () => {
    clearTimeout(collectTimer.current);
    collectTimer.current = undefined;
    const ranked = rankCandidates(tally.current);
    tally.current = new Map();
    setCollecting(false);
    if (ranked.length > 0) setPick(ranked);
  };
  const onScan = (text: string, format?: string) => {
    if (pick) return;
    const reads = countRead(tally.current, text, format);
    if (reads === 0) return;
    if (reads >= ENOUGH_READS) {
      finish();
      return;
    }
    if (collectTimer.current === undefined) {
      setCollecting(true);
      setNoCode(false);
      collectTimer.current = setTimeout(finish, COLLECT_MS);
    }
  };
  const look = (text: string) => {
    const code = asCode(text);
    if (!code) return;
    setNoCode(false);
    setPick([{ code, reads: 1 }]);
    setChosen(code);
  };
  const again = () => {
    setPick(null);
    setChosen(null);
    setTyped('');
  };
  const state = useQrScanner(video, cameraAllowed && pick === null, onScan, 'barcode');
  const onType = (event: FormEvent) => {
    event.preventDefault();
    look(typed);
  };

  const lookupOf = (code: string) =>
    lookups[pick?.findIndex((candidate) => candidate.code === code) ?? -1];
  const top = pick?.[0];
  const topLookup = top ? lookupOf(top.code) : undefined;
  // One code, or one that clearly won and is known, is shown at once; else the codes to pick.
  const shown =
    chosen ??
    (top &&
    pick &&
    (pick.length === 1 ||
      (isDominant(pick) && !topLookup?.error && topLookup?.data?.kind !== 'unknown'))
      ? top.code
      : null);
  const shownLookup = shown ? lookupOf(shown) : undefined;
  const others = pick?.filter((candidate) => candidate.code !== shown) ?? [];

  return (
    <Sheet title={t('barcode.title')} onClose={onClose}>
      <div data-testid="barcode-sheet" className="flex flex-col gap-3 pb-2 text-sm">
        {pick && shown ? (
          shownLookup?.data ? (
            <>
              <Found find={shownLookup.data} onDone={onClose} onAgain={again} />
              {others.length > 0 && (
                <div className="flex flex-col gap-2 pt-2">
                  <p className="text-muted">{t('barcode.otherCodes')}</p>
                  <ul className="flex flex-col gap-2">
                    {others.map((candidate) => (
                      <CandidateRow
                        key={candidate.code}
                        candidate={candidate}
                        lookup={lookupOf(candidate.code) ?? { isPending: true, error: null }}
                        onPick={() => setChosen(candidate.code)}
                      />
                    ))}
                  </ul>
                </div>
              )}
            </>
          ) : shownLookup?.error ? (
            <>
              <SourceIssue>{shownLookup.error.message}</SourceIssue>
              <button
                type="button"
                onClick={again}
                className={`${TAP_BUTTON} bg-background self-start`}
              >
                <ScanBarcode aria-hidden size={16} />
                {t('barcode.scanAgain')}
              </button>
            </>
          ) : (
            <p role="status" className="flex items-center gap-2 py-6">
              <LoaderCircle aria-hidden size={18} className="animate-spin" />
              {t('barcode.looking', { code: shown })}
            </p>
          )
        ) : pick ? (
          <div className="flex flex-col gap-2">
            <p className="font-medium">{t('barcode.pickTitle')}</p>
            <ul className="flex flex-col gap-2">
              {pick.map((candidate) => (
                <CandidateRow
                  key={candidate.code}
                  candidate={candidate}
                  lookup={lookupOf(candidate.code) ?? { isPending: true, error: null }}
                  onPick={() => setChosen(candidate.code)}
                />
              ))}
            </ul>
            <button
              type="button"
              onClick={again}
              className={`${TAP_BUTTON} bg-background self-start`}
            >
              <ScanBarcode aria-hidden size={16} />
              {t('barcode.scanAgain')}
            </button>
          </div>
        ) : (
          <>
            <p className="text-muted">{t('barcode.hint')}</p>
            {cameraAllowed && state !== 'noCamera' && state !== 'denied' && (
              <div className="relative aspect-[4/3] w-full overflow-hidden rounded-xl bg-black">
                <video
                  ref={video}
                  muted
                  playsInline
                  aria-label={t('link.cameraView')}
                  className="size-full object-cover"
                />
                {/* Where to hold the barcode. */}
                <span
                  aria-hidden
                  className="absolute inset-x-[12%] top-1/2 h-px -translate-y-1/2 bg-red-500/80"
                />
                {state === 'starting' && (
                  <LoaderCircle
                    aria-hidden
                    size={28}
                    className="absolute inset-0 m-auto animate-spin text-white"
                  />
                )}
                {collecting && (
                  <p
                    role="status"
                    className="absolute inset-x-0 bottom-0 flex items-center justify-center gap-2 bg-black/60 py-2 text-white"
                  >
                    <LoaderCircle aria-hidden size={16} className="animate-spin" />
                    {t('barcode.reading')}
                  </p>
                )}
              </div>
            )}
            {(state === 'denied' || (android && camera?.state === 'blocked')) && android && (
              <button
                type="button"
                onClick={() => requestPermission.mutate('camera')}
                className={`${TAP_BUTTON} bg-background self-start`}
              >
                <Camera aria-hidden size={16} />
                {t('link.allowCamera')}
              </button>
            )}
            <button
              type="button"
              onClick={() => picture.current?.click()}
              className={`${TAP_BUTTON} bg-background self-start`}
            >
              <ImageUp aria-hidden size={16} />
              {t('barcode.fromPicture')}
            </button>
            <input
              ref={picture}
              type="file"
              accept="image/*"
              aria-label={t('barcode.fromPicture')}
              className="hidden"
              onChange={(event) => {
                const file = event.target.files?.[0];
                event.target.value = '';
                if (!file) return;
                void readPicture(file)
                  .catch(() => [])
                  .then((reads) => {
                    const found: Tally = new Map();
                    reads.forEach((read) => countRead(found, read.text, read.format));
                    const ranked = rankCandidates(found);
                    if (ranked.length > 0) {
                      setNoCode(false);
                      setPick(ranked);
                    } else setNoCode(true);
                  });
              }}
            />
            {noCode && <p className="text-muted">{t('barcode.noCode')}</p>}
            <form onSubmit={onType} className="flex items-center gap-2">
              <TextField
                value={typed}
                onChange={setTyped}
                inputMode="numeric"
                aria-label={t('barcode.typed')}
                placeholder={t('barcode.typed')}
                className={`${SHEET_FIELD} flex-1`}
                inputClassName={SHEET_INPUT}
              />
              <button
                type="submit"
                disabled={!asCode(typed)}
                className={`${TAP_BUTTON} bg-background disabled:opacity-50`}
              >
                {t('barcode.look')}
              </button>
            </form>
          </>
        )}
      </div>
    </Sheet>
  );
};

/** Search's scan button, opening the barcode sheet. */
export const ScanBarcodeButton: FC = () => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  return (
    <>
      <button
        type="button"
        aria-label={t('barcode.scan')}
        onClick={() => setOpen(true)}
        className="text-muted hover:text-foreground grid size-10 shrink-0 place-items-center rounded-full"
      >
        <ScanBarcode aria-hidden size={20} />
      </button>
      {open && <BarcodeSheet onClose={() => setOpen(false)} />}
    </>
  );
};
