# cueseg

A timed transcript into topic chapters.

Part of [catena](../../README.md).

## What it does

Finds where a recording changes subject, and writes one [`segment`
record](../../docs/records.md#segment--one-topic-chapter) per chapter: a start, an end, a
title taken from the chapter's own first sentence, and an excerpt.

No model and no network. The text is hashed into a small lexical vector a
window at a time, and a topic boundary is where two consecutive windows stop
resembling each other.

## When you would use it

- **Chapter marks for a player.** A three-hour recording with no marks is
  unnavigable; the boundaries this finds are exactly the marks a scrubber
  needs.
- **Feeding a summariser one subject at a time.** A whole transcript is too
  much context and a paragraph is not enough; a chapter is the unit that gives
  a useful answer.
- **Navigation in a search result.** Knowing which chapter a hit falls in is
  often more useful than the timestamp alone.
- **Titles before anybody has written any.** The extracted titles are meant to
  be replaced by something better later, and to be usable until they are.

## Install

```sh
cargo install --git https://git.pumpin.online/funkpower/catena cueseg
```

## Example

A two-hour recording of two subjects:

```
$ cueseg --duration 7200 transcript.ndjson
{"v":1,"start":0.0,"end":1190.6666666666667,"title":"The Mill Stood At The Northern End Of","excerpt":"the mill stood at the northern end of the valley where the fall of the river was steepest and the ice came latest…"}
{"v":1,"start":1190.6666666666667,"end":2381.3333333333335,"title":"On One Household And The Benefit Was Spread","excerpt":"on one household and the benefit was spread across four hundred families…"}
{"v":1,"start":2381.3333333333335,"end":3572.0,"title":"Stood At The Northern End Of The Valley","excerpt":"stood at the northern end of the valley where the fall of the river was steepest…"}
{"v":1,"start":3572.0,"end":4479.0,"title":"Steepest And The Ice Came Latest Its Wheel","excerpt":"steepest and the ice came latest its wheel was undershot which was wasteful…"}
{"v":1,"start":4479.0,"end":5386.0,"title":"It From The Journals Of Either House Parliament","excerpt":"it from the journals of either house parliament dissolved itself again amid furious objections…"}
{"v":1,"start":5386.0,"end":6293.0,"title":"At All Which Left The Committee With No","excerpt":"at all which left the committee with no record of the vote…"}
{"v":1,"start":6293.0,"end":7200.0,"title":"House Parliament Dissolved Itself Again Amid Furious Objections","excerpt":"house parliament dissolved itself again amid furious objections from every bench…"}
```

Straight from a subtitle file:

```sh
cuenorm episode.vtt | cueseg --duration 7245 > chapters.ndjson
```

As chapter marks for a player:

```sh
cueseg --duration 7245 transcript.ndjson | jq -r '"\(.start | floor)\t\(.title)"'
```

## Options

| Option | Effect |
|---|---|
| `--duration <SECONDS>` | How long the recording is |
| `--man` | Write this page as a manual page, in roff |

Without it, the duration is taken from the end of the last cue. That is
earlier than the end of the file whenever a recording ends in music or
silence, and a chapter that claims to run to the end of the transcript rather
than the end of the file is a mark that lands in the wrong place.

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Done |
| `64` | The command line was wrong, or the duration was not a length |
| `65` | The cue stream could not be read |
| `74` | The file could not be read |

## How it finds a boundary

1. **Windows.** Cues are gathered into stretches of about seventy-five
   seconds — long enough to have a subject, short enough that a boundary
   lands within a minute of the real one.
2. **Vectors.** Each window's content words are hashed into a small vector of
   counts. Filler, discourse markers and the vague nouns that carry a
   conversation are dropped: leaving them in makes every window resemble every
   other.
3. **Valleys.** A boundary is where two consecutive windows resemble each
   other less than their neighbours do, and less than the recording's own
   average by half a standard deviation. Valleys are taken deepest first, each
   at least a chapter's length from the ones already taken.
4. **A floor and a ceiling.** Between four and twenty chapters, targeting one
   every fifteen minutes. A chapter mark every two minutes is not navigation,
   it is noise; and a chapter that runs too long is no more navigable than no
   chapter at all, so an over-long span is divided evenly whether or not the
   text suggested a break there.
5. **A fallback.** A recording where nothing stands out is cut evenly, which
   is better than one chapter of three hours.

## Input

Either the cue stream [`cuenorm`](../cuenorm/README.md) writes, or a subtitle file.

## As a library

```rust
for chapter in cueseg::segment(&cues, 7245.0) {
    println!("{:.0}s {}", chapter.start, chapter.title);
}
```

## See also

- [`cuenorm`](../cuenorm/README.md) — what to run before this.
- [`cuepack`](../cuepack/README.md) — the other thing to do with a cue stream.
