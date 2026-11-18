import { commands } from '@raygun/bindings';
import type { BarcodeFind } from '@raygun/bindings';
import { useMutation } from '@tanstack/react-query';
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

/**
 * Search's scan button (v2.0.0): a CD's or record's barcode opens its album (MusicBrainz finds
 * the release, the music sources the album), a book's ISBN finds the book (Open Library, then
 * the book sources). The camera reads the code (Android, or a desktop webcam), or a picture of
 * it, or the number typed.
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
  const lookup = useMutation({
    mutationFn: (code: string) => unwrapResult(commands.barcodeLookup(code)),
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
  const look = (text: string) => {
    const code = asCode(text);
    if (!code || lookup.isPending) return;
    setNoCode(false);
    lookup.mutate(code);
  };
  const scanning = cameraAllowed && !lookup.isPending && !lookup.data;
  const state = useQrScanner(video, scanning, look, 'barcode');
  const onType = (event: FormEvent) => {
    event.preventDefault();
    look(typed);
  };

  return (
    <Sheet title={t('barcode.title')} onClose={onClose}>
      <div data-testid="barcode-sheet" className="flex flex-col gap-3 pb-2 text-sm">
        {lookup.data ? (
          <Found
            find={lookup.data}
            onDone={onClose}
            onAgain={() => {
              lookup.reset();
              setTyped('');
            }}
          />
        ) : lookup.isPending ? (
          <p role="status" className="flex items-center gap-2 py-6">
            <LoaderCircle aria-hidden size={18} className="animate-spin" />
            {t('barcode.looking', { code: lookup.variables })}
          </p>
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
                void readPicture(file).then((code) => {
                  if (code) look(code);
                  else setNoCode(true);
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
        {lookup.error && <SourceIssue>{lookup.error.message}</SourceIssue>}
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
