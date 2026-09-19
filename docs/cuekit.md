# cuekit

The cue model, and the three subtitle formats it is read from.

Back to [the index](README.md).

## What it is

A library. Three formats go in — SubRip, WebVTT and SubStation Alpha — and
one [`cue`](records.md#cue--one-line-of-a-transcript) stream comes out.
Everything downstream works on cues and never has to know which of the three a
recording arrived with.

[`cuenorm`](cuenorm.md) is a command over this.

## When you would use it

- **Reading subtitles in your own program**, without shelling out.
- **Adding a fourth format**, or handling one of these three differently.
- **Anywhere timed text is the input** and the format is whatever somebody
  happened to have.

## Reading a file

```rust
let cues = cuekit::parse(&content);
```

The format is worked out from the content, not from the name, because a
subtitle file is named for the format somebody expected it to be in. When it
is already known:

```rust
use cuekit::Format;

let cues = cuekit::parse_as(&content, Format::WebVtt);
assert_eq!(cuekit::detect(&content), Format::WebVtt);
```

The parsers are also available directly — `cuekit::srt::parse` (which reads
WebVTT too) and `cuekit::ass::parse`.

## The cue

```rust
use cuekit::Cue;

let cue = Cue::new(12.5, 15.0, "the mill stood at the northern end");
assert_eq!(cue.seconds(), 2.5);
```

Deliberately smaller than a [chunk](records.md#chunk--a-retrievable-passage).
A cue is what a subtitle file holds — a few seconds, often half a sentence —
and turning cues into passages worth retrieving is a later stage's job.

`seconds()` is negative if the file says a cue ends before it starts, which
some do.

## Merging the repeats

```rust
let merged = cuekit::merge_repeats(&cues);
```

Consecutive cues with identical text become one, keeping the earliest start
and the latest end.

Automatic captioning emits a rolling window: the same line appears in cue
after cue as the window slides, so a ten-minute stretch of speech can arrive
as three times as many cues as there are sentences. Indexed as they come,
every one of those is a separate hit for the same words at almost the same
timestamp.

## Cleaning a cue's text

```rust
assert_eq!(cuekit::text::clean("<v Host>[Music] The mill stood"), "The mill stood");
```

Styling and speaker tags go; bracketed sound cues — `[Music]`, `[Applause]`,
`[inaudible]` — go with them, because they are not speech and a search that
matches them matches every recording with a theme tune.

## Timestamps

```rust
assert_eq!(cuekit::srt::timestamp("00:01:23,456"), 83.456);
assert_eq!(cuekit::ass::timestamp("0:01:23.45"), 83.45);
```

Both return `NaN` rather than an error for something that is not a timestamp,
which is what lets a malformed block be dropped whole instead of guessed at.

`cuekit::jsnum` holds the number parsing these use. It reproduces the two
JavaScript conversions the original parser was written against, which disagree
about almost everything an ill-formed timestamp can contain — an empty string
is zero to one of them and not a number to the other. Reproducing both is what
makes a malformed file drop the same cues here as there.

## See also

- [`cuenorm`](cuenorm.md) — the command over this.
- [`cuepack`](cuepack.md) and [`cueseg`](cueseg.md) — what reads cues.
- [The records](records.md) — the cue shape on the wire.
