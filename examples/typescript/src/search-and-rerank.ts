// Two searches, reconciled — the app-side half of hybrid retrieval.
//
// Fusion is the one thing in this suite worth calling in process rather than
// as a subprocess: by the time a hit reaches your code it is already yours,
// and serialising it to hand to a program and back is work for nothing.
//
//     npm install
//     npm run search
//
// The hits below stand in for two queries you would run against your own
// store — pgvector and a text index, Elasticsearch and a KNN service, or a
// vector database and whatever else. What they came from does not matter
// here; only where each of them placed does.

import { fuseHits, type Hit } from "catena";

// `payload` is whatever you want carried alongside — rrf hands it back
// untouched, so a hit can arrive here with the row it came from attached.
const hit = (id: string, channel: string, rank: number, score: number): Hit => ({
  v: 1,
  id,
  channel,
  rank,
  score,
  payload: {},
});

// Nearest in the embedding space. Finds the passage about the fall of Rome
// for a query about decline, sharing not one word with it.
const byMeaning: Hit[] = [
  hit("p39", "vec", 1, 0.21),
  hit("p127", "vec", 2, 0.24),
  hit("p462", "vec", 3, 0.26),
  hit("p177", "vec", 4, 0.29),
];

// Nearest by the words themselves. Finds the passage that says the word
// where the vectors drifted to a paraphrase — and finds the front matter,
// which says "Roman Empire" more often than any chapter of the book does.
const byWord: Hit[] = [
  hit("p0", "fts", 1, 0.68),
  hit("p462", "fts", 2, 0.51),
  hit("p8", "fts", 3, 0.44),
  hit("p71", "fts", 4, 0.4),
];

const text: Record<string, string> = {
  p0: "The Project Gutenberg eBook of History of the Decline and Fall…",
  p8: "In imitation of his classical models, Gibbon places Rome as the…",
  p39: "In the second century of the Christian Æra, the empire of Rome…",
  p71: "Tacit. Annal. iv. 5. Those who fix a regular proportion…",
  p127: "It was by such institutions that the nations of the empire…",
  p177: "Among many other passages, see Pliny on the extent of the…",
  p462: "The received calculation of Varro assigns to the foundation…",
};

for (const [label, hits] of [["by meaning", byMeaning], ["by word", byWord]] as const) {
  console.log(`\n== ${label}`);
  for (const hit of hits) {
    console.log(`   ${hit.rank}. [${hit.id}] ${hit.score.toFixed(2)}  ${text[hit.id]}`);
  }
}

// The two score columns above mean different things — a cosine distance and
// a ts_rank_cd — and no scaling makes them comparable. So the ranks are used
// and the scores thrown away.
console.log("\n== fused");
for (const hit of fuseHits([...byMeaning, ...byWord], undefined, undefined, 5)) {
  console.log(`   ${hit.rank}. [${hit.id}] ${hit.score.toFixed(4)}  ${text[hit.id]}`);
}

// One channel can be trusted more than the other where a corpus warrants it.
console.log("\n== fused, trusting the vectors twice as much");
for (const hit of fuseHits([...byMeaning, ...byWord], undefined, { vec: 2 }, 5)) {
  console.log(`   ${hit.rank}. [${hit.id}] ${hit.score.toFixed(4)}  ${text[hit.id]}`);
}

console.log(`
   Reciprocal rank fusion scores a hit by where it placed, not by what it
   scored: 1/(k + rank) from each list that found it, added up. p462 is
   third in one list and second in the other and wins, because agreement
   between two different ways of looking is evidence; p0 is first in one
   and absent from the other, and drops.

   This is fusion, and it is often called re-ranking, but a second thing
   wears the same name. Retrieval uses a bi-encoder: the passage and the
   query are embedded separately, which is what lets passage vectors be
   computed once and indexed — and is also what limits it, since nothing in
   a passage's vector knows what was asked.

   A cross-encoder reads the query and the passage together and scores the
   pair. Far more accurate, far too slow to run over a corpus, so it runs
   over the twenty or fifty candidates retrieval already found. The two
   stack: retrieve both ways, fuse as above, then hand the top of the fused
   list to a cross-encoder for the final order. Nothing here loads a model,
   so that step is yours — the fused list is what you would hand it.`);
