#!/usr/bin/env bash
# Starts a Linux build of Raygun in a virtual display with a data folder of its own (RAYGUN_HOME)
# and checks it's still running 40 seconds later without a crash log, keeping a screenshot of the
# screen at that moment. Used by the release workflow for the AppImage, the deb and the Arch package.
#
# Usage: start-test.sh <screenshot.png> <program> [arguments...]
set -u
shot=$(realpath -m "$1")
shift
mkdir -p "$(dirname "$shot")"
home=$(mktemp -d)
export RAYGUN_HOME="$home" SHOT="$shot"
# WebKitGTK's DMA-BUF renderer needs a GPU the virtual display doesn't have.
export WEBKIT_DISABLE_DMABUF_RENDERER=1

xvfb-run -a -s "-screen 0 1280x800x24" dbus-run-session -- bash -c '
  "$@" &
  app=$!
  sleep 40
  import -window root "$SHOT" 2>/dev/null || echo "No screenshot was taken"
  if kill -0 "$app" 2>/dev/null; then
    kill "$app"
    wait "$app" 2>/dev/null
    exit 0
  fi
  wait "$app"
  echo "::error::Raygun stopped by itself (exit $?)"
  exit 1
' start-test "$@"
status=$?

find "$home/logs" -maxdepth 1 -type f -exec tail -n 30 {} + 2>/dev/null
if [ -d "$home/logs/crashes" ] && [ -n "$(ls -A "$home/logs/crashes")" ]; then
  echo "::error::Raygun wrote a crash log"
  cat "$home"/logs/crashes/*
  exit 1
fi
exit "$status"
