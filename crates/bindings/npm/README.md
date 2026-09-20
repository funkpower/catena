# catena

Turn a pile of files into a searchable corpus: chunking, scrubbing, subtitles
and rank fusion, as WebAssembly.

```sh
npm install catena
```

```js
import { chunkMarkdown, scrub, countTokens, fuseHits } from "catena";

const chunks = chunkMarkdown(markdown, 512, 64);
chunks[0].page;      // 41
chunks[0].chapter;   // 'III. The Second Estate'

const clean = scrub(rawMarkdown, true);
countTokens(clean);

fuseHits([...vectorHits, ...textHits], undefined, { vec: 2 }, 20);
```

Records come back as plain objects — the same documents the command-line
tools write, with the same field names and the same stability promise.
Nothing has to be freed.

`splitCollection` throws when the document is one work rather than a
collection, which is the usual answer:

```js
let works;
try {
  works = splitCollection(markdown);
} catch (why) {
  works = [{ idx: 0, title, text: markdown }];
}
```

## What is here

| | |
|---|---|
| `chunkMarkdown` | Markdown into passages that carry their page, chapter and role |
| `scrub` | Converted markdown in, clean markdown out |
| `countTokens` | Tokens, by the encoding the budgets count in |
| `parseSubtitles` | SubRip, WebVTT or ASS into cues |
| `packCues` | Cues into passages, keeping the times they were said at |
| `segmentCues` | A timed transcript into topic chapters |
| `cueWindows` | The windows `segmentCues` measures, to embed |
| `splitCollection` | One markdown file into the works inside it |
| `fuseHits` | Several ranked lists into one |
| `identify` | What a file is, from its first bytes |
| `seriesOf` | The series a book belongs to, and which volume this is |
| `parseIdentifier` | A DOI, ISSN, ISBN or arXiv id, read and checked |

The tools that read files or call another program are not here: a browser has
neither, and a binding that pretends otherwise fails at run time rather than
at compile time. Those stay command-line tools.

## Node and the web

```js
import { chunkMarkdown } from "catena";        // resolves per environment
import { chunkMarkdown } from "catena/node";   // or pick one
import { chunkMarkdown } from "catena/web";
```

Two builds because the loaders differ: node reads the `.wasm` off disk at
import, the web build takes a URL a bundler rewrites.

## Licence

MPL-2.0. Source: <https://git.pumpin.online/funkpower/catena>
