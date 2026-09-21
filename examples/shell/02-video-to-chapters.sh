#!/bin/sh
# A recording to chapter marks, citable passages, and a copy that plays.
#
# Takes the captions a video already carries, normalises them to one cue
# stream, and turns that into chapter marks for a player and passages worth
# embedding — each keeping the second it was said at. Then asks whether the
# file itself would play on a phone, and prints the one ffmpeg call that
# fixes it.
#
#     examples/shell/02-video-to-chapters.sh [url] [work-dir]
#
# The default is a NASA upload: a work of the US government, so public
# domain, and captioned by a person rather than by a model. Any URL yt-dlp
# can reach works, and any local subtitle file works too — skip to the
# cuenorm line and feed it your own.
#
# Needs jq, ffmpeg, and yt-dlp with impersonation support, which YouTube now
# requires before it will hand over captions:
#
#     pipx install "yt-dlp[default,curl-cffi]"
#
# and the catena commands on PATH.
set -eu

url="${1:-https://www.youtube.com/watch?v=h0wLZJeYGxw}"
work="${2:-./work}"
mkdir -p "$work"

# Captions written by a person where there are any, and a model's otherwise.
# Automatic captions roll — each line is re-sent as it scrolls up the screen —
# so the same sentence arrives three times and every passage says it thrice.
if [ -z "$(find "$work" -name '*.vtt' -print -quit 2>/dev/null)" ]; then
	echo "fetching captions"
	yt-dlp --skip-download --write-subs --sub-langs 'en.*' --sub-format vtt \
		--output "$work/%(id)s.%(ext)s" "$url" >/dev/null 2>&1 ||
		yt-dlp --skip-download --write-auto-subs --sub-langs 'en.*' --sub-format vtt \
			--output "$work/%(id)s.%(ext)s" "$url" >/dev/null
fi

# Where several tracks came back, the smallest is the written one: a rolling
# track repeats every line and is four or five times the size.
subs="$(find "$work" -name '*.vtt' -exec ls -S {} + | tail -1)"
[ -n "$subs" ] || { echo "no captions on that video; try another URL" >&2; exit 1; }
echo "   captions: $(basename "$subs")"

echo
echo "== cues"
cuenorm "$subs" > "$work/cues.ndjson"
jq -s -r '"   \(length) cues over \((map(.end) | max) / 60 | floor)m\((map(.end) | max) % 60 | floor)s"' "$work/cues.ndjson"
jq -r '"   [\(.start | floor)s] \(.text[0:62])"' "$work/cues.ndjson" | head -3

# Where the subject changes, found from the words alone — no model, no
# network. The titles come out of each chapter's own first sentence, and are
# meant to be replaced by something better later.
echo
echo "== chapters"
cueseg < "$work/cues.ndjson" > "$work/chapters.ndjson"
jq -r '"   \(.start | floor)s–\(.end | floor)s  \(.title)"' "$work/chapters.ndjson"

# Cues packed to a size worth embedding, each carrying the seconds it spans,
# so a search hit can be played back at the moment it was said.
echo
echo "== passages"
cuepack < "$work/cues.ndjson" > "$work/passages.ndjson"
jq -s -r '"   \(length) passages, \(map(.tokens) | add / length | floor) tokens on average"' "$work/passages.ndjson"
jq -s -r '.[0] | "   [\(.start_time | floor)s–\(.end_time | floor)s] \(.text[0:150])…"' "$work/passages.ndjson"

echo
echo "== chapter marks, as a player would take them"
jq -r '"\(.start | floor) \(.title)"' "$work/chapters.ndjson" |
	awk '{ t = $1; $1 = ""; printf "   %02d:%02d:%02d%s\n", t/3600, (t%3600)/60, t%60, $0 }'

# And the file itself. A recording indexed but unplayable is half a job: a
# phone will not open VP9 in a WebM container, whatever the transcript says.
# Twenty seconds of it is enough to judge the streams, and spares the
# download.
echo
echo "== would it play on a phone"
video="$(find "$work" -type f \( -name '*.webm' -o -name '*.mkv' -o -name '*.mp4' \) -print -quit)"
if [ -z "$video" ]; then
	echo "   fetching twenty seconds of it"
	yt-dlp -f 'worstvideo[ext=webm]+worstaudio[ext=webm]/worst' \
		--download-sections '*0-20' --force-keyframes-at-cuts \
		--output "$work/%(id)s.%(ext)s" "$url" >/dev/null 2>&1 || true
	video="$(find "$work" -type f \( -name '*.webm' -o -name '*.mkv' -o -name '*.mp4' \) -print -quit)"
fi
[ -n "$video" ] || { echo "   nothing to judge"; exit 0; }

avplan --json "$video" | jq -r '
	"   \(.streams.container): \(.streams.video) video, \(.streams.audio // "no") audio",
	(if .playable then "   plays as it stands" else (.reasons[] | "   · \(.)") end)'

# Exit 0 means there is a plan to print; 1 means the file already plays. The
# encoder is whatever this machine actually has — videotoolbox on a Mac,
# NVENC or VAAPI on a Linux box, libx264 where there is no card. Nothing is
# run: converting is expensive and irreversible enough to read first.
echo
if plan="$(avplan --output "$work/for-phone.mp4" "$video")"; then
	echo "   the one call that fixes it:"
	echo
	printf '   %s\n' "$plan"
	echo
	echo "   run it with:  eval \"\$(avplan --output out.mp4 FILE)\""
else
	echo "   nothing to convert"
fi

echo
echo "written to $work/"
