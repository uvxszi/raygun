#!/usr/bin/env bash
# Starts a Linux build of Raygun in a virtual display with a data folder of its own (RAYGUN_HOME)
# and checks that it says its start went well (Raygun logs that after 20 healthy seconds) within
# 90 seconds, is still running then, and wrote no crash record; keeps a screenshot of the screen.
# Used by the release workflow for the AppImage, the deb and the Arch package.
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
  for _ in $(seq 1 45); do
    sleep 2
    kill -0 "$app" 2>/dev/null || break
    grep -qs "the start went well" "$RAYGUN_HOME"/logs/raygun.* && break
  done
  # A moment for the page to draw before the screenshot.
  sleep 5
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
# Crash records are `<time>-<kind>.txt`; the folder also holds the start markers.
if compgen -G "$home/logs/crashes/*.txt" > /dev/null; then
  echo "::error::Raygun wrote a crash log"
  cat "$home"/logs/crashes/*.txt
  exit 1
fi
if ! grep -qs "the start went well" "$home"/logs/raygun.*; then
  echo "::error::Raygun's log doesn't say its start went well"
  exit 1
fi
exit "$status"
