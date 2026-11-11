import { commands } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Bluetooth, Cable, Check, Headphones, Monitor, Speaker, Usb, Volume2 } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { listOutputs, outputSink, setOutputSink } from '../engine/outputSink';
import { usePlatform } from '../lib/usePlatform';
import { unwrapResult } from '../lib/commandResult';
import { TAP_NOTE, TAP_OPTION } from './tapTargets';

const ICONS = {
  speaker: Speaker,
  wired: Headphones,
  bluetooth: Bluetooth,
  usb: Usb,
  hdmi: Monitor,
};

const ROW = `${TAP_OPTION} hover:bg-background`;

/** How often Android is asked again while the list shows: songs, picks and headphones move it. */
const FOLLOW_MS = 1500;

type Output = { id: string; name: string; kind: string };

/**
 * Where this device's sound comes out (M46.43): Automatic (the system's choice, which follows
 * headphones as they're plugged in), or one output: the speaker, wired, Bluetooth or USB
 * headphones, HDMI; on desktop every output the system has. On Android the output the playing
 * song actually goes to is marked, and a note says when Android kept it elsewhere than picked,
 * or when YouTube's player plays it (that player goes where Android sends sound).
 */
export const AudioOutputs: FC = () => {
  const { t } = useTranslation();
  const platform = usePlatform();
  const android = platform === 'android';
  const client = useQueryClient();
  const native = useQuery({
    queryKey: ['audioOutputs'],
    queryFn: () => commands.audioOutputs(),
    enabled: android,
    refetchInterval: FOLLOW_MS,
  });
  const web = useQuery({
    queryKey: ['audioOutputs', 'web'],
    queryFn: listOutputs,
    enabled: platform !== undefined && !android,
  });
  const [webChosen, setWebChosen] = useState(outputSink);
  const pick = useMutation({
    mutationFn: async (id: string | null) => {
      if (android) await unwrapResult(commands.audioOutputSet(id));
      else {
        setOutputSink(id ?? '');
        setWebChosen(id ?? '');
      }
    },
    onSettled: () => void client.invalidateQueries({ queryKey: ['audioOutputs'] }),
  });

  const outputs: Output[] = android
    ? (native.data?.outputs ?? [])
    : (web.data ?? [])
        .filter((output) => output.id !== '')
        .map((output, index) => ({
          id: output.id,
          name: output.name || t('outputs.numbered', { n: index + 1 }),
          kind: /bluetooth|airpods|buds/i.test(output.name)
            ? 'bluetooth'
            : /headphone|headset/i.test(output.name)
              ? 'wired'
              : /hdmi|display/i.test(output.name)
                ? 'hdmi'
                : 'speaker',
        }));
  const chosen = android ? (native.data?.chosen ?? null) : webChosen || null;
  if (outputs.length === 0) return null;
  // A phone's own speaker and wired headphones go by what they are (Android names them after
  // the phone model); Bluetooth, USB and desktop outputs by their own names.
  const name = (output: Output) =>
    (android && (output.kind === 'speaker' || output.kind === 'wired')) || !output.name
      ? t(`outputs.kind.${android ? `${output.kind}Phone` : output.kind}`, {
          defaultValue: t(`outputs.kind.${output.kind}`, { defaultValue: output.kind }),
        })
      : output.name;
  const nameOf = (id: string | null | undefined) => {
    const output = outputs.find((each) => each.id === id);
    return output ? name(output) : null;
  };
  const routed = android ? (native.data?.routed ?? null) : null;
  const system = android ? (native.data?.system ?? null) : null;
  const inYouTube = android && (native.data?.web ?? false);
  const refused = android && (native.data?.refused ?? false);
  // Where the sound goes without a pick: where it plays now, else where Android would send it.
  const automaticNow = chosen === null ? nameOf(routed ?? system) : null;
  const note = pick.isError
    ? t('outputs.failed')
    : refused
      ? nameOf(routed)
        ? t('outputs.refused', { name: nameOf(routed) })
        : t('outputs.refusedSomewhere')
      : inYouTube && chosen !== null
        ? nameOf(system)
          ? t('outputs.youTube', { name: nameOf(system) })
          : t('outputs.youTubeSomewhere')
        : null;

  return (
    <div className="flex flex-col pl-6">
      <ul aria-label={t('outputs.title')} data-testid="audio-outputs" className="flex flex-col">
        <li>
          <button
            type="button"
            aria-pressed={chosen === null}
            disabled={pick.isPending}
            onClick={() => pick.mutate(null)}
            className={ROW}
          >
            <Cable aria-hidden size={20} className="text-muted shrink-0" />
            <span className="min-w-0 flex-1">
              <span className="block truncate">{t('outputs.automatic')}</span>
              {automaticNow && (
                <span className={TAP_NOTE}>{t('outputs.now', { name: automaticNow })}</span>
              )}
            </span>
            {chosen === null && <Check aria-hidden size={20} className="text-primary shrink-0" />}
          </button>
        </li>
        {outputs.map((output) => {
          const Icon = ICONS[output.kind as keyof typeof ICONS] ?? Speaker;
          const on = chosen === output.id;
          const playing = routed === output.id;
          return (
            <li key={output.id}>
              <button
                type="button"
                aria-pressed={on}
                disabled={pick.isPending}
                onClick={() => pick.mutate(output.id)}
                className={ROW}
              >
                <Icon
                  aria-hidden
                  size={20}
                  className={`shrink-0 ${on ? 'text-primary' : 'text-muted'}`}
                />
                <span className="min-w-0 flex-1">
                  <span className={`block truncate ${on ? 'text-primary font-medium' : ''}`}>
                    {name(output)}
                  </span>
                  {playing && (
                    <span
                      data-testid="output-playing"
                      className={`${TAP_NOTE} flex items-center gap-1`}
                    >
                      <Volume2 aria-hidden size={14} className="shrink-0" />
                      {t('outputs.playingHere')}
                    </span>
                  )}
                </span>
                {on && <Check aria-hidden size={20} className="text-primary shrink-0" />}
              </button>
            </li>
          );
        })}
      </ul>
      {note && (
        <p role="status" data-testid="output-note" className="text-muted px-3 pt-1 pb-2 text-sm">
          {note}
        </p>
      )}
    </div>
  );
};
