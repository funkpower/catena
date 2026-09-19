// Exercise the built package the way a consumer would.
import {
  chunkMarkdown, scrub, countTokens, parseSubtitles, packCues,
  splitCollection, fuseHits, identify,
} from "./dist/node/catena.cjs";

const md = "# Chapter One\n\n<!-- Page 41 -->\n\nThe obligation ran both ways, and contemporaries said so plainly whenever they were asked about it at all.\n";

const chunks = chunkMarkdown(md, undefined, undefined, 10);
console.log("chunk:     ", { idx: chunks[0].idx, page: chunks[0].page, chapter: chunks[0].chapter, tokens: chunks[0].tokens });

console.log("scrub:     ", JSON.stringify(scrub("# Heading\n\n\n\nbody <b>text</b>\n")));
console.log("tokens:    ", countTokens("a passage of prose"));

const cues = parseSubtitles("1\n00:00:01,000 --> 00:00:04,000\nthe mill stood at the northern end\n\n2\n00:00:04,000 --> 00:00:08,000\nof the valley where the road turned\n\n");
console.log("cues:      ", cues.length, cues[0]);
console.log("packed:    ", packCues(cues, 5)[0].text);

const hits = [
  { v: 1, id: "c81a", channel: "vec", rank: 1, score: 0.83, payload: {} },
  { v: 1, id: "c81a", channel: "fts", rank: 3, score: 11.2, payload: {} },
  { v: 1, id: "d42f", channel: "fts", rank: 1, score: 14.0, payload: {} },
];
console.log("fused:     ", fuseHits(hits, undefined, undefined, 2).map((h) => [h.id, +h.score.toFixed(5)]));
console.log("weighted:  ", fuseHits(hits, undefined, { vec: 2 }, 2).map((h) => [h.id, +h.score.toFixed(5)]));

console.log("identify:  ", identify("book.pdf", new TextEncoder().encode("%PDF-1.4\n")).kind);

try { splitCollection(md); } catch (why) { console.log("split throws:", String(why).slice(0, 52)); }

// The types the package promises.
const types = await import("node:fs").then((fs) => fs.readFileSync("./dist/web/catena.d.ts", "utf8"));
for (const name of ["Chunk", "Cue", "Section", "Hit", "Doc"]) {
  if (!types.includes(`interface ${name}`)) throw new Error(`no ${name} in the declarations`);
}
console.log("types:      Chunk, Cue, Section, Hit, Doc all declared");
