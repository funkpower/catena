/**
 * Parse SubRip the way the podcast archive was indexed, for comparison.
 *
 * The three functions below are the originals, copied out of the indexer
 * verbatim and with their type annotations dropped — the module they live in
 * opens a database connection when it loads, which an oracle must not do.
 *
 *     node oracle/cuenorm.mjs < case.srt
 *
 * Writes one JSON cue per line.
 */

/** Parse an SRT timestamp (00:01:23,456) into seconds. */
function parseSrtTimestamp(ts) {
  // SRT format: HH:MM:SS,mmm  (comma separates ms)
  const cleaned = ts.trim().replace(",", ".");
  const parts = cleaned.split(":");
  if (parts.length === 3) {
    return Number(parts[0]) * 3600 + Number(parts[1]) * 60 + parseFloat(parts[2]);
  }
  if (parts.length === 2) {
    return Number(parts[0]) * 60 + parseFloat(parts[1]);
  }
  return NaN;
}

/** Parse an SRT file content into cues. */
function parseSrt(content) {
  const cues = [];
  const blocks = content.split(/\n\n+/);

  for (const block of blocks) {
    const lines = block.trim().split("\n");
    const tsLine = lines.find((l) => l.includes("-->"));
    if (!tsLine) continue;

    const [startStr, endStr] = tsLine.split("-->").map((s) => s.trim());
    const startTime = parseSrtTimestamp(startStr);
    const endTime = parseSrtTimestamp(endStr);
    if (isNaN(startTime) || isNaN(endTime)) continue;

    const tsIdx = lines.indexOf(tsLine);
    const text = lines
      .slice(tsIdx + 1)
      .join(" ")
      .replace(/<[^>]+>/g, "")
      .replace(/\[.*?\]/g, "")
      .trim();

    if (text) {
      cues.push({ startTime, endTime, text });
    }
  }

  return cues;
}

/** Merge consecutive cues with identical text. */
function deduplicateCues(cues) {
  if (cues.length === 0) return [];

  const result = [{ ...cues[0] }];

  for (let i = 1; i < cues.length; i++) {
    const prev = result[result.length - 1];
    const curr = cues[i];

    if (curr.text === prev.text) {
      prev.endTime = Math.max(prev.endTime, curr.endTime);
    } else {
      result.push({ ...curr });
    }
  }

  return result;
}

const chunks = [];
for await (const chunk of process.stdin) chunks.push(chunk);
const content = Buffer.concat(chunks).toString("utf-8");

for (const cue of deduplicateCues(parseSrt(content))) {
  process.stdout.write(
    JSON.stringify({ start: cue.startTime, end: cue.endTime, text: cue.text }) + "\n",
  );
}
