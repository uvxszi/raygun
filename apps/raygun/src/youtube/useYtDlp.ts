import { commands, events } from '@raygun/bindings';
import type { YtDlpKind } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

import { unwrapResult } from '../lib/commandResult';

const STATUS = ['ytdlp', 'status'] as const;
const JOBS = ['ytdlp', 'jobs'] as const;

/** Looks like a YouTube address: worth asking Rust about (M46.47). */
export const looksLikeYouTube = (text: string) =>
  /(^|\/\/|\.)(youtube\.com|youtu\.be|youtube-nocookie\.com)\//i.test(text.trim());

/** What a typed YouTube address points at, with its title when YouTube says. */
export const useYouTubeLink = (text: string) => {
  const trimmed = text.trim();
  return useQuery({
    queryKey: ['youtubeLink', trimmed],
    queryFn: () => unwrapResult(commands.youtubeLink(trimmed)),
    enabled: looksLikeYouTube(trimmed),
    staleTime: 60 * 60_000,
  });
};

/** yt-dlp and FFmpeg on this device. */
export const useYtDlpStatus = (enabled = true) =>
  useQuery({
    queryKey: STATUS,
    queryFn: () => unwrapResult(commands.ytdlpStatus()),
    enabled,
  });

/**
 * yt-dlp's downloads; follows its events while shown. An event names the job that changed, or
 * nothing when yt-dlp or FFmpeg was being fetched.
 */
export const useYtDlpJobs = () => {
  const client = useQueryClient();
  useEffect(() => {
    const unlisten = events.ytDlpEvent.listen(({ payload }) => {
      void client.invalidateQueries({ queryKey: payload ? JOBS : STATUS });
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [client]);
  return useQuery({ queryKey: JOBS, queryFn: () => commands.ytdlpJobs() });
};

export const useYtDlpActions = () => {
  const client = useQueryClient();
  const refresh = () => void client.invalidateQueries({ queryKey: ['ytdlp'] });
  return {
    download: useMutation({
      mutationFn: ({ url, title, kind }: { url: string; title: string; kind: YtDlpKind }) =>
        unwrapResult(commands.ytdlpDownload(url, title, kind)),
      onSettled: refresh,
    }),
    cancel: useMutation({
      mutationFn: (id: string) => commands.ytdlpCancel(id),
      onSettled: refresh,
    }),
    reveal: useMutation({
      mutationFn: (id: string) => unwrapResult(commands.ytdlpReveal(id)),
    }),
    install: useMutation({
      mutationFn: () => unwrapResult(commands.ytdlpInstall()),
      onSettled: refresh,
    }),
    installFfmpeg: useMutation({
      mutationFn: () => unwrapResult(commands.ytdlpInstallFfmpeg()),
      onSettled: refresh,
    }),
    /** Stops fetching yt-dlp or FFmpeg at once; nothing of it is kept. */
    cancelInstall: useMutation({
      mutationFn: () => commands.ytdlpCancelInstall(),
      onSettled: refresh,
    }),
  };
};
