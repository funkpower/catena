// Checked, not run: every line here must compile against the declarations the
// package ships. A binding whose types say `any` is one TypeScript cannot
// check, and nothing else would notice.
import {
  chunkMarkdown, scrub, countTokens, parseSubtitles, packCues,
  splitCollection, fuseHits, identify, seriesOf, parseIdentifier, segmentCues, cueWindows,
  findCitations, citationPrompt, mergeCitationReply,
  type Chunk, type Cue, type Section, type Hit, type Doc, type SeriesProposal, type Identifier, type Segment,
  type Reference,
} from "./dist/web/catena.js";

const chunks: Chunk[] = chunkMarkdown("# One\n\nbody\n", 512, 64, 50, 768);
const page: number | null = chunks[0].page;
const chapter: string | null = chunks[0].chapter;
const flags: string[] = chunks[0].flags;

const clean: string = scrub("# One\n", true);
const tokens: number = countTokens(clean);

const cues: Cue[] = parseSubtitles("1\n00:00:01,000 --> 00:00:04,000\nline\n\n");
const start: number = cues[0].start;
const packed: Chunk[] = packCues(cues, 75, 512);

const chapters: Segment[] = segmentCues(cues, 3600);
const toEmbed: string[] = cueWindows(cues);
segmentCues(cues, 3600, toEmbed.map(() => [1, 0]));
const chapterTitle: string = chapters[0].title;
segmentCues(cues);

const works: Section[] = splitCollection("# One\n\nbody\n");
const slug: string = works[0].slug;

const hits: Hit[] = [{ v: 1, id: "a", channel: "vec", rank: 1, score: 0.5, payload: {} }];
const fused: Hit[] = fuseHits(hits, 60, { vec: 2, fts: 1 }, 10);

const doc: Doc = identify("book.pdf", new Uint8Array([0x25, 0x50]));
const kind: string = doc.kind;
const notes: string[] = doc.notes;

// Optional arguments really are optional.
chunkMarkdown("x");
parseSubtitles("x");
packCues(cues);
fuseHits(hits);

const proposal: SeriesProposal | undefined = seriesOf("The Philokalia Vol 3");
const volume: number | undefined = proposal?.volume;
seriesOf("Aquinas (Companion)", "Cambridge");

const references: Reference[] = findCitations("Smith, John. *A Title*. Oxford: Clarendon Press, 1998.");
const unsettled: string[] = references[0].unsettled;
const ask: string | undefined = citationPrompt(references);
const filled: Reference[] = mergeCitationReply(references, "{}");
void unsettled;
void ask;
void filled;

const ident: Identifier | undefined = parseIdentifier("10.1000/182");
const checksum: boolean | null | undefined = ident?.checksum;

export const checked = [volume, checksum, chapterTitle, page, chapter, flags, clean, tokens, start, packed, slug, fused, kind, notes];
