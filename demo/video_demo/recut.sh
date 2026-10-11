#!/bin/sh
# Re-cut assets/clip.mp4 (750 frames @30fps = 25.000s) from the fresh
# e2e/videos/demo_human_flow.webm. Cut table below; retimed beats are in
# .plans/62 (shot table) — verify beats with `frame` extracts before cutting.
set -e
cd "$(dirname "$0")"

SRC=../../e2e/videos/demo_human_flow.webm
OUT=assets/clip.mp4
TMP=tmp_cut

rm -rf "$TMP" && mkdir -p "$TMP"

SEGMENTS="4.5 60
12.0 70
16.5 65
20.6 55
24.5 80
27.8 60
39.0 55
44.5 80
54.5 90
64.2 135"

n=0
echo "$SEGMENTS" | while read -r start frames; do
  n=$((n + 1))
  seg=$(printf 'seg%02d' "$n")
  printf '%s: %ss +%s frames\n' "$seg" "$start" "$frames"
  # fps=30 normalizes to CFR; tpad clones the last frame if the source ends
  ffmpeg -loglevel error -y -ss "$start" -i "$SRC" \
    -vf "fps=30,tpad=stop_mode=clone:stop_duration=5" \
    -frames:v "$frames" -an "$TMP/$seg.mp4"
done

for f in "$TMP"/seg*.mp4; do printf "file '%s'\n" "$(basename "$f")" >> "$TMP/list.txt"; done

ffmpeg -loglevel error -y -f concat -safe 0 -i "$(pwd)/$TMP/list.txt" \
  -c:v libx264 -preset slow -crf 18 -pix_fmt yuv420p -r 30 -an "$OUT"

rm -rf "$TMP"
ffprobe -v error -select_streams v:0 -count_frames \
  -show_entries stream=nb_read_frames,width,height,avg_frame_rate \
  -of default=nw=1 "$OUT"
