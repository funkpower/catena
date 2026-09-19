# cuenorm

Any subtitle format in, one normalised cue stream out.

Part of [catena](../../README.md).

## What it does

Reads a subtitle file — SubRip, WebVTT or SubStation Alpha — and writes one
[`cue` record](../../docs/records.md#cue--one-line-of-a-transcript) per line: a start, an
end, and what was said.

The format is worked out from the content, not from the name, because a
subtitle file is named for the format somebody expected it to be in.

## When you would use it

- **At the front of anything that reads recordings.** Everything downstream
  works on cues and never has to know which of the three formats a recording
  arrived with.
- **Cleaning up automatic captioning.** It repeats each line across
  overlapping windows, so the same sentence arrives three or four times over;
  indexed as they come, every one is a separate hit for the same words at
  almost the same timestamp, which buries everything else in a result list.
- **Getting markup and sound cues out of the way.** `<v Speaker>` tags,
  styling, `[Music]`, `[Applause]` — none of it is speech, and a search that
  matches `[Music]` matches every recording with a theme tune.

## Install

```sh
cargo install --git https://git.pumpin.online/funkpower/catena cuenorm
```

## Example

A WebVTT file with a speaker tag, a sound cue, and a repeated line:

```
$ cat episode.vtt
WEBVTT

00:00:01.000 --> 00:00:04.500
<v Host>The mill stood at the northern end of the valley,

00:00:04.500 --> 00:00:08.000
where the fall of the river was steepest and the ice came latest.

00:00:08.000 --> 00:00:11.000
[Music]

00:00:11.000 --> 00:00:15.500
Its wheel was undershot, which was wasteful,

00:00:15.000 --> 00:00:19.000
Its wheel was undershot, which was wasteful,

00:00:19.000 --> 00:00:24.000
and nobody replaced it for two hundred years.
```

```
$ cuenorm episode.vtt
{"v":1,"start":1.0,"end":4.5,"text":"The mill stood at the northern end of the valley,"}
{"v":1,"start":4.5,"end":8.0,"text":"where the fall of the river was steepest and the ice came latest."}
{"v":1,"start":11.0,"end":19.0,"text":"Its wheel was undershot, which was wasteful,"}
{"v":1,"start":19.0,"end":24.0,"text":"and nobody replaced it for two hundred years."}
```

The speaker tag is gone, `[Music]` left no cue at all, and the repeated line
became one cue spanning both — `11.0` to `19.0`, the earliest start and the
latest end.

In a pipeline:

```sh
cuenorm episode.vtt | cuepack --seconds 75 > passages.ndjson
cuenorm episode.srt | cueseg --duration 7245 > chapters.ndjson
```

## Options

| Option | Effect |
|---|---|
| `--keep-repeats` | Keep consecutive repeats instead of merging them |
| `--man` | Write this page as a manual page, in roff |

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Done |
| `64` | The command line was wrong |
| `74` | The file could not be read |

Bytes that are not UTF-8 are replaced rather than refused. A subtitle file
downloaded from anywhere is as likely to be Latin-1, and a cue with one wrong
character still has its timing and most of its words.

## The formats

| Format | Recognised by |
|---|---|
| WebVTT | The `WEBVTT` line at the front |
| SubStation Alpha | A `[Script Info]` or `[Events]` section |
| SubRip | Neither of the above |

SubStation Alpha events are rows under a `Format:` line that names the
columns, so the column order is read rather than assumed. Override blocks
(`{\an8}`), line breaks (`\N`) and drawing commands are handled: a drawing
command is a shape, not speech, and reads as a string of coordinates if it is
kept.

A WebVTT cue line carries positioning settings after the end time, which are
ignored — that is also what lets an old SubRip file with screen coordinates
after its timestamps read at all.

## As a library

```rust
let cues = cuekit::merge_repeats(&cuekit::parse(&content));
for cue in &cues {
    println!("{:.1}–{:.1} {}", cue.start, cue.end, cue.text);
}
```

See [`cuekit`](../cuekit/README.md) for the parsers one at a time.

## See also

- [`cuepack`](../cuepack/README.md) — cues into passages worth embedding.
- [`cueseg`](../cueseg/README.md) — cues into topic chapters.
- [`cuekit`](../cuekit/README.md) — the library this is a command over.
