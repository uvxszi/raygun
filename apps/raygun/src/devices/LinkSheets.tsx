import { commands } from '@raygun/bindings';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Camera, Copy, LoaderCircle, RefreshCw } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import type { FC, FormEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { usePlatform } from '../lib/usePlatform';
import { useLocalNetworkGate } from '../network/useLocalNetworkGate';
import { canAsk, usePermissions, useRequestPermission } from '../permissions/usePermissions';
import { Problem } from '../settings/Problem';
import { useRemoteChange, useRemoteStatus } from '../settings/remoteQueries';
import { Sheet } from '../shell/Sheet';
import { SHEET_FIELD, SHEET_INPUT, TextField } from '../shell/TextField';
import { useQrScanner } from './qrScanner';

const BUTTON =
  'flex items-center gap-1.5 rounded-full px-4 py-2 text-sm font-semibold disabled:opacity-50';

/** The name a link code carries, to say who is being asked. */
const nameIn = (text: string) => {
  const query = text.trim().split('?')[1];
  return query ? (new URLSearchParams(query).get('name') ?? '') : '';
};

const isLinkCode = (text: string) => text.trim().startsWith('raygun:link?');

const refreshDevices = (client: ReturnType<typeof useQueryClient>) => {
  void client.invalidateQueries({ queryKey: ['connect'] });
  void client.invalidateQueries({ queryKey: ['remote'] });
  void client.invalidateQueries({ queryKey: ['sync'] });
};

/**
 * This device's link QR code (M46.47): its address and a one-time code. The other device scans
 * it; then this device asks "Link with …?" as usual. Opening it lets devices find this one.
 */
export const ShowLinkCode: FC<{ onClose: () => void }> = ({ onClose }) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const remote = useRemoteStatus();
  const network = useLocalNetworkGate('remote');
  const [copied, setCopied] = useState(false);
  const [shown, setShown] = useState(false);
  const newCode = useRemoteChange(async () => {
    await unwrapResult(commands.devicesSetSharing(true));
    return unwrapResult(commands.remoteNewCode());
  });
  const open = () =>
    void network.guard(null, () =>
      newCode.mutate(undefined, {
        onSuccess: () => setShown(true),
        onSettled: () => refreshDevices(client),
      }),
    );

  useEffect(() => {
    open();
    // A fresh code each time the sheet opens.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const data = remote.data;
  const text = data?.linkText;
  const used = shown && !newCode.isPending && data && !text;
  return (
    <Sheet title={t('link.showCode')} onClose={onClose}>
      <div data-testid="show-link-code" className="flex flex-col items-center gap-3 pb-2 text-sm">
        {text && data.linkQrSvg && (
          <>
            <img
              data-testid="link-qr"
              alt={t('link.qrAlt')}
              src={`data:image/svg+xml;utf8,${encodeURIComponent(data.linkQrSvg)}`}
              className="size-56 rounded-xl bg-white p-2"
            />
            <ul className="text-muted flex flex-col items-center text-xs">
              <li>{t('link.scanOnOther')}</li>
              <li>{t('link.codeOnce')}</li>
            </ul>
            <button
              type="button"
              onClick={() => void navigator.clipboard?.writeText(text).then(() => setCopied(true))}
              className="text-primary flex items-center gap-1 text-xs font-semibold"
            >
              <Copy aria-hidden size={14} />
              {copied ? t('link.copied') : t('link.copyCode')}
            </button>
          </>
        )}
        {newCode.isPending && <LoaderCircle aria-hidden size={28} className="animate-spin" />}
        {used && <p className="text-muted">{t('link.codeUsed')}</p>}
        {newCode.error && <Problem title={t('link.codeFailed')} detail={newCode.error.message} />}
        {!newCode.isPending && (
          <button
            type="button"
            onClick={() => {
              setCopied(false);
              open();
            }}
            className={`${BUTTON} bg-background`}
          >
            <RefreshCw aria-hidden size={14} />
            {t('link.newCode')}
          </button>
        )}
      </div>
      {network.sheet}
    </Sheet>
  );
};

/**
 * Scans another device's link QR code with the camera (M46.47), or takes its text pasted (a
 * computer without a camera); then that device asks "Link with …?".
 */
export const ScanLinkCode: FC<{ onClose: () => void }> = ({ onClose }) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const platform = usePlatform();
  const android = platform === 'android';
  const permissions = usePermissions();
  const requestPermission = useRequestPermission();
  const network = useLocalNetworkGate('connect');
  const video = useRef<HTMLVideoElement>(null);
  const [pasted, setPasted] = useState('');
  const [wrongCode, setWrongCode] = useState(false);
  const link = useMutation({
    mutationFn: (text: string) => unwrapResult(commands.connectLinkCode(text)),
    onSuccess: onClose,
    onSettled: () => refreshDevices(client),
  });

  // Android asks for the camera through Settings → Permissions' flow first; the WebView then
  // gets it without asking again.
  const camera = permissions.data?.find((permission) => permission.kind === 'camera');
  // Without a camera row (an older plugin) the WebView asks by itself.
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

  const scanning = cameraAllowed && !link.isPending && !link.isSuccess;
  const submit = (text: string) => {
    if (!isLinkCode(text)) {
      setWrongCode(true);
      return;
    }
    setWrongCode(false);
    void network.guard(null, () => link.mutate(text.trim()));
  };
  const state = useQrScanner(video, scanning && !link.error, (text) => {
    if (isLinkCode(text) && !link.isPending) submit(text);
  });
  const onPaste = (event: FormEvent) => {
    event.preventDefault();
    submit(pasted);
  };
  const asking = link.variables ? nameIn(link.variables) : '';

  return (
    <Sheet title={t('link.scanCode')} onClose={onClose}>
      <div data-testid="scan-link-code" className="flex flex-col gap-3 pb-2 text-sm">
        {cameraAllowed && state !== 'noCamera' && state !== 'denied' && !link.isPending && (
          <div className="relative aspect-square w-full overflow-hidden rounded-xl bg-black">
            <video
              ref={video}
              muted
              playsInline
              aria-label={t('link.cameraView')}
              className="size-full object-cover"
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
        {state === 'noCamera' && cameraAllowed && (
          <p className="text-muted">{t('link.noCamera')}</p>
        )}
        {(state === 'denied' || (android && camera?.state === 'blocked')) && (
          <div className="flex flex-col gap-2">
            <p className="text-muted">{t('link.cameraBlocked')}</p>
            {android && (
              <button
                type="button"
                onClick={() => requestPermission.mutate('camera')}
                className={`${BUTTON} bg-background self-start`}
              >
                <Camera aria-hidden size={14} />
                {t('link.allowCamera')}
              </button>
            )}
          </div>
        )}
        {link.isPending && (
          <p role="status" className="flex items-center gap-2 py-6">
            <LoaderCircle aria-hidden size={18} className="animate-spin" />
            {t('link.waiting', { name: asking || t('link.thatDevice') })}
          </p>
        )}
        {link.error && (
          <Problem title={t('link.linkFailed', { name: asking || t('link.thatDevice') })} />
        )}
        {/* Rust says what to do (scan again, show a new code) in plain words. */}
        {link.error && <p className="text-muted text-xs">{link.error.message}</p>}
        {link.error && (
          <button
            type="button"
            onClick={() => link.reset()}
            className={`${BUTTON} bg-background self-start`}
          >
            {t('link.scanAgain')}
          </button>
        )}
        <form onSubmit={onPaste} className="flex items-center gap-2">
          <TextField
            value={pasted}
            onChange={setPasted}
            autoCapitalize="off"
            autoCorrect="off"
            spellCheck={false}
            aria-label={t('link.pasteCode')}
            placeholder={t('link.pasteCode')}
            className={`${SHEET_FIELD} min-w-0 flex-1`}
            inputClassName={SHEET_INPUT}
          />
          <button
            type="submit"
            disabled={!pasted.trim() || link.isPending}
            className={`${BUTTON} bg-primary text-background`}
          >
            {t('link.link')}
          </button>
        </form>
        {wrongCode && <p className="text-primary text-xs">{t('link.notACode')}</p>}
      </div>
      {network.sheet}
    </Sheet>
  );
};
