import { commands, events } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

import { unwrapResult } from '../lib/commandResult';

const FOLLOWS = ['music', 'follows'];
const NEWS = ['music', 'releaseNews'];

/** Keeps follows and release news current from Rust (M46.42). */
const useFollowEvents = () => {
  const client = useQueryClient();
  useEffect(() => {
    const unlisten = events.artistReleasesEvent.listen(() => {
      void client.invalidateQueries({ queryKey: FOLLOWS });
      void client.invalidateQueries({ queryKey: NEWS });
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [client]);
};

/** Followed artists, the latest followed first (M46.42). */
export const useFollowedArtists = () => {
  useFollowEvents();
  return useQuery({
    queryKey: FOLLOWS,
    queryFn: () => unwrapResult(commands.artistsFollowed()),
  });
};

/** New releases found for followed artists (M46.42). */
export const useReleaseNews = () => {
  useFollowEvents();
  return useQuery({
    queryKey: NEWS,
    queryFn: () => unwrapResult(commands.artistReleaseNews()),
  });
};

export const useFollowActions = () => {
  const client = useQueryClient();
  const refresh = () => {
    void client.invalidateQueries({ queryKey: FOLLOWS });
    // Quick access changed too (M46.41).
    void client.invalidateQueries({ queryKey: ['settings'] });
  };
  return {
    follow: useMutation({
      mutationFn: ({ name, imageUrl }: { name: string; imageUrl: string | null }) =>
        unwrapResult(commands.artistFollow(name, imageUrl)),
      onSettled: refresh,
    }),
    unfollow: useMutation({
      mutationFn: (name: string) => unwrapResult(commands.artistUnfollow(name)),
      onSettled: refresh,
    }),
    alerts: useMutation({
      mutationFn: ({ name, on }: { name: string; on: boolean }) =>
        unwrapResult(commands.artistSetAlerts(name, on)),
      onSettled: refresh,
    }),
  };
};

/** Normalized, as Rust compares artist names. */
export const sameArtist = (left: string, right: string) =>
  left.trim().toLocaleLowerCase() === right.trim().toLocaleLowerCase();
