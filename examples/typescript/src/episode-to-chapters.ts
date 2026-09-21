// A recording's captions into chapter marks and citable passages.
//
// What a player needs from an episode: somewhere to jump to, and passages
// that carry the second they were said at so a search result can be played
// rather than just quoted.
//
//     npm install
//     npm run chapters -- path/to/captions.vtt
//
// With no argument it reads the sample below, which is a few cues of a NASA
// broadcast — a work of the US government, so public domain.

import { readFileSync } from "node:fs";
import { parseSubtitles, segmentCues, packCues, countTokens } from "catena-corpus";

const SAMPLE = `WEBVTT

00:00:00.000 --> 00:00:04.200
Hey there, and welcome to NASA's Curious Universe.

00:00:04.200 --> 00:00:08.900
Our universe is a wild and wonderful place, and in this
show, NASA is your tour guide.

00:00:08.900 --> 00:00:15.100
Today we are going to talk about how we find planets
around other stars, which is harder than it sounds.

00:00:15.100 --> 00:00:21.800
A planet is small and dark and its star is enormous and
bright, so you almost never see the planet itself.

00:00:21.800 --> 00:00:28.400
Instead you watch the star, and you wait for it to dim
by about one part in ten thousand.

00:00:28.400 --> 00:00:34.000
That dimming is a transit, and it is how most of the
planets we know about were found.
`;

const path = process.argv[2];
const content = path ? readFileSync(path, "utf8") : SAMPLE;

// One cue stream, whatever the format was. `keepRepeats` is off, which is
// what throws away the re-sent lines of a rolling automatic caption track —
// leave it on and every passage says each sentence three times.
const cues = parseSubtitles(content);
const runsTo = Math.max(...cues.map((cue) => cue.end));
console.log(`== ${cues.length} cues over ${Math.round(runsTo)}s`);
for (const cue of cues.slice(0, 3)) {
  console.log(`   [${cue.start.toFixed(1)}s] ${cue.text.slice(0, 58)}`);
}

// Where the subject changes, from the words alone — no model, no network.
// The titles come out of each chapter's own first sentence and are meant to
// be replaced by something better later.
const chapters = segmentCues(cues);
console.log(`\n== ${chapters.length} chapter(s)`);
for (const chapter of chapters) {
  console.log(`   ${stamp(chapter.start)}  ${chapter.title}`);
}

// Cues packed to a size worth embedding, each keeping the seconds it spans.
const passages = packCues(cues);
console.log(`\n== ${passages.length} passage(s)`);
for (const passage of passages) {
  console.log(
    `   [${stamp(passage.start_time ?? 0)}–${stamp(passage.end_time ?? 0)}] ` +
      `${passage.tokens} tokens`,
  );
  console.log(`      ${passage.text.slice(0, 92)}…`);
}

// What a player would take: a cue sheet, and a deep link per passage.
console.log("\n== what the player gets");
for (const chapter of chapters) {
  console.log(`   ${stamp(chapter.start)} ${chapter.title}`);
}
console.log("\n== what a search result would link to");
for (const passage of passages.slice(0, 2)) {
  const at = Math.floor(passage.start_time ?? 0);
  console.log(`   ?t=${at}  ${passage.text.slice(0, 64)}…`);
}

// The same token count the chunker used, so a budget worked out here and a
// budget enforced there agree.
console.log(`\n   ${countTokens(cues.map((c) => c.text).join(" "))} tokens of transcript in all`);

function stamp(seconds: number): string {
  const whole = Math.floor(seconds);
  const mm = String(Math.floor(whole / 60)).padStart(2, "0");
  const ss = String(whole % 60).padStart(2, "0");
  return `${String(Math.floor(whole / 3600)).padStart(2, "0")}:${mm}:${ss}`;
}
