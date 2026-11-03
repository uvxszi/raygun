import { commands } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Bluetooth, Cable, Check, Headphones, Monitor, Speaker, Usb } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { listOutputs, outputSink, setOutputSink } from '../engine/outputSink';
import { usePlatform } from '../lib/usePlatform';
import { unwrapResult } from '../lib/commandResult';

const ICONS = {
  speaker: Speaker,
  wired: Headphones,
  bluetooth: Bluetooth,
  usb: Usb,
  hdmi: Monitor,
};

const ROW =
  'hover:bg-background flex w-full items-center gap-3 rounded-xl px-2 py-2 text-left text-sm';

/**
 * Where this device's sound comes out (M46.43): Automatic (the system's choice, which follows
 * headphones as they're plugged in), or one output: the speaker, wired, Bluetooth or USB
 * headphones, HDMI; on desktop every output the system has.
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

  const outputs: { id: string; name: string; kind: string }[] = android
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
  const name = (output: { name: string; kind: string }) =>
    (android && (output.kind === 'speaker' || output.kind === 'wired')) || !output.name
      ? t(`outputs.kind.${android ? `${output.kind}Phone` : output.kind}`, {
          defaultValue: t(`outputs.kind.${output.kind}`, { defaultValue: output.kind }),
        })
      : output.name;

  return (
    <ul aria-label={t('outputs.title')} data-testid="audio-outputs" className="flex flex-col pl-8">
      <li>
        <button
          type="button"
          aria-pressed={chosen === null}
          disabled={pick.isPending}
          onClick={() => pick.mutate(null)}
          className={ROW}
        >
          <Cable aria-hidden size={16} className="text-muted" />
          <span className="flex-1">{t('outputs.automatic')}</span>
          {chosen === null && <Check aria-hidden size={16} className="text-primary" />}
        </button>
      </li>
      {outputs.map((output) => {
        const Icon = ICONS[output.kind as keyof typeof ICONS] ?? Speaker;
        const on = chosen === output.id;
        return (
          <li key={output.id}>
            <button
              type="button"
              aria-pressed={on}
              disabled={pick.isPending}
              onClick={() => pick.mutate(output.id)}
              className={ROW}
            >
              <Icon aria-hidden size={16} className={on ? 'text-primary' : 'text-muted'} />
              <span className={`min-w-0 flex-1 truncate ${on ? 'text-primary font-medium' : ''}`}>
                {name(output)}
              </span>
              {on && <Check aria-hidden size={16} className="text-primary" />}
            </button>
          </li>
        );
      })}
    </ul>
  );
};
