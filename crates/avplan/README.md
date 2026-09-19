# avplan

Says whether a recording plays as it is, and prints the one `ffmpeg` call that
fixes it when it does not.

Part of [catena](../../README.md).

## What it does

Reads what a file's streams actually are and does the least that fixes them.
An already-H.264 picture is copied rather than re-encoded, which is the
difference between forty minutes and forty seconds.

It runs nothing. What comes out is a command to read, pipe, or run — which is
the point: a re-encode is expensive and the original is irreplaceable.

## When you would use it

- **Before a flight.** Convert the six files that need it rather than the
  ninety in the directory.
- **Sorting a download folder.** A file that plays needs nothing; knowing
  which ones those are saves the rest of the work.
- **Diagnosing a video that opens to a black screen** — or plays with no
  sound at all, which is what Opus in an MP4 looks like on a device that
  cannot decode it.

## Install

```sh
cargo install --git https://git.pumpin.online/funkpower/catena avplan
```

Needs `ffprobe` and `ffmpeg` on `PATH`. They ship together.

## Example

A VP9/Opus recording in a container nothing will open:

```
$ avplan lecture.webm
ffmpeg -nostdin -y -i lecture.webm -c:v h264_videotoolbox -q:v 60 -profile:v high -pix_fmt yuv420p -c:a aac -b:a 160k -ac 2 -movflags +faststart lecture.h264.mp4
```

Nothing to do:

```
$ avplan talk.mp4
$ echo $?
1
```

An H.264 picture in a Matroska container, with the sound in the wrong codec.
The picture is copied — only the sound is re-encoded, and the file is rewritten
into a container that opens:

```
$ avplan film.mkv
ffmpeg -nostdin -y -i film.mkv -c:v copy -c:a aac -b:a 160k -ac 2 -movflags +faststart film.h264.mp4
```

Why, rather than what:

```
$ avplan --json film.mkv | jq -c '{playable, reasons, encoder}'
{"playable":false,"reasons":["the sound is opus","a .mkv container cannot be opened"],"encoder":"copy"}
```

Converting everything in a directory that needs it:

```sh
for f in *.webm *.mkv; do avplan "$f" | xargs -d '\n' ffmpeg; done
```

`avplan` writes nothing for a file that already plays, so `xargs` runs nothing
for it either.

## Options

| Option | Effect |
|---|---|
| `--json` | Emit a record — the streams, the reasons, the arguments |
| `--output <PATH>` | Where the converted copy goes. Defaults to `<name>.h264.mp4` beside the original |
| `--encoder <NAME>` | Use this encoder rather than the best one installed |
| `--vaapi-device <PATH>` | The render node to encode on. Defaults to `/dev/dri/renderD128` |

## Exit codes

| Code | Meaning |
|---|---|
| `0` | It needs converting, and the command is on stdout |
| `1` | It plays as it is |
| `2` | `ffprobe` could not read it |
| `64` | The command line was wrong |
| `69` | `ffmpeg` or `ffprobe` is not installed, or has no H.264 encoder |
| `74` | The file is not there |

Exit `1` is the good answer, not a failure.

## How it decides

Three questions, and any one of them failing means the file needs work.

**The picture.** H.264 or HEVC plays; anything else — VP9, AV1, VP8, MPEG-4 —
does not. This is the only expensive question: a picture that already plays is
copied through untouched.

**The sound.** AAC plays. Opus in an MP4 is the quiet failure worth naming,
because the file opens, the picture runs, and there is simply no sound.

**The container.** `.mp4`, `.m4v` and `.mov` can be opened. Matroska cannot,
whatever is inside it — which is why a `.mkv` full of H.264 and AAC still has
to be rewritten, though rewriting it is nearly free.

## Encoders

The encoder is found rather than configured: `ffmpeg -encoders` is asked what
this machine has, and the best of them is taken.

| Encoder | Where |
|---|---|
| `h264_nvenc` | NVIDIA |
| `h264_videotoolbox` | Apple silicon and recent Macs |
| `h264_vaapi` | AMD and Intel graphics |
| `libx264` | Software. Always there, and slower by a wide margin |

Hardware first, because the difference on a two-hour recording is minutes
against most of an hour. Each gets asked for roughly the same quality in
whichever of the four quality dialects it speaks:

```
$ avplan --encoder libx264 lecture.webm
ffmpeg -nostdin -y -i lecture.webm -c:v libx264 -preset veryfast -crf 23 -pix_fmt yuv420p -c:a aac -b:a 160k -ac 2 -movflags +faststart lecture.h264.mp4
```

An `ffmpeg` built without any of the four cannot produce H.264 at all, and
`avplan` says so with exit `69` rather than printing a command that would
fail.

## As a library

```rust
let streams = avplan::Streams {
    video: Some("vp9".into()),
    audio: Some("opus".into()),
    container: avplan::container_of("lecture.webm"),
};
let plan = avplan::plan(
    "lecture.webm",
    &streams,
    Some(avplan::Encoder::Libx264),
    "lecture.h264.mp4",
    avplan::DEFAULT_VAAPI_DEVICE,
);
assert!(!plan.playable);
```

`video_plays`, `audio_plays` and `container_plays` answer the three questions
on their own, for anything that needs one of them without the rest.

## See also

- [`cuenorm`](../cuenorm/README.md) — the subtitles that go with the recording.
- [The record contract](../../docs/records.md) — what `--json` emits.
