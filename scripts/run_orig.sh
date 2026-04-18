#!/usr/bin/env sh

(
  cd data/mm6 || exit 1

  GDK_BACKEND=x11 \
  gamescope -w 640 -h 480 -W 1920 -H 1080 -- \
  env WINEPREFIX="$HOME/.wine-mm6" \
  wine-vanilla-11.6 explorer /desktop=MM6,640x480 MM6.exe
)
