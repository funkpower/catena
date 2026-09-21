#!/bin/sh
# A book file to passages worth embedding, and what its notes cite.
#
# The starting point is Gibbon's first volume from Project Gutenberg: public
# domain, a real table of contents, and a few hundred pages of footnotes.
#
#     examples/shell/01-book-to-passages.sh [work-dir]
#
# Needs pandoc and jq, and the catena commands on PATH.
set -eu

work="${1:-./work}"
mkdir -p "$work"
book="$work/gibbon.epub"

if [ ! -s "$book" ]; then
	echo "fetching Gibbon vol. 1 from Project Gutenberg"
	curl -sSL --max-time 120 -o "$book" \
		"https://www.gutenberg.org/cache/epub/731/pg731.epub"
fi

# What the file actually is. Extensions lie, and a `.epub` that is really a
# zip of page scans fails four stages later, in a way that looks like a bug
# in the converter.
echo
echo "== what it is"
booksniff "$book" | jq -r '"   \(.kind), \(.bytes) bytes"'

# A PDF gets one more question first: exit 0 from pdftriage means the text
# layer is not worth reading and the file has to be OCRed before any of this.
if [ "$(booksniff "$book" | jq -r .kind)" = "pdf" ] && pdftriage --quiet "$book"; then
	echo "   the text layer is garbled; OCR it before going on"
	exit 0
fi

# What the file already says about itself — the publisher's own metadata,
# which beats anything guessed from a filename, and costs no lookup.
echo
echo "== what it says about itself"
bookmeta "$book" | jq -r '"   \(.title)\n   \(.authors | join("; "))\n   \(.language // "?") · \(.date // "no date")"'

# The chapters the book declares, rather than any inferred from the text.
echo
echo "== the contents it declares"
tocfind "$book" --outline | sed -n '6,10p' | sed 's/^/   /'
echo "   … $(tocfind "$book" | wc -l | tr -d ' ') entries"

# Convert, scrub, chunk. `commonmark` rather than `commonmark_x`, because the
# extended flavour writes anchors as bracketed spans and they end up in the
# passages. The scrub is what takes out the rest of the converter's leavings.
echo
echo "== passages"
pandoc -f epub -t commonmark --wrap=none "$book" -o "$work/book.md"
mdscrub < "$work/book.md" > "$work/clean.md"
chunkmd < "$work/clean.md" > "$work/chunks.ndjson"

jq -s -r '
	"   \(length) passages over \([.[].chapter] | unique | length) chapters",
	"   roles: \([.[].role] | group_by(.) | map("\(length) \(.[0])") | join(", "))",
	"   \([.[] | select(.flags | index("footnote_citations"))] | length) carry citation material"
' "$work/chunks.ndjson"

echo
echo "   a passage, with where it came from:"
jq -s -r '[.[] | select(.role == "body" and .chapter != null)][40]
	| "   [\(.idx)] \(.tokens) tokens · \(.chapter)\n   \(.text[0:150])…"' \
	"$work/chunks.ndjson"

# The notes. A passage flagged `footnote_citations` is a page of them, and
# each is read on its own — the flag says which passages are worth the ask,
# which is the whole reason chunkmd sets it.
echo
echo "== what the notes cite"
: > "$work/references.ndjson"
jq -c 'select(.flags | index("footnote_citations")) | .text' "$work/chunks.ndjson" \
	| while IFS= read -r line; do
		printf '%s' "$line" | jq -r . | citefind >> "$work/references.ndjson" || true
	done

jq -s -r '
	"   \(length) entries read",
	"   \([.[] | select(.title)] | length) name a work, \([.[] | select(.authors | length > 0)] | length) name an author",
	"   \([.[] | select(.unsettled | length > 0)] | length) came back with something unread"
' "$work/references.ndjson"

echo
echo "   Gibbon cites in the classical short form — \"Tacit. Annal. xiii. 5.\" —"
echo "   so most entries name the passage rather than a book, and say so:"
jq -r 'select(.unsettled | length > 0) | "   \(.text[0:58]) → unread: \(.unsettled | join(", "))"' \
	"$work/references.ndjson" | head -4

echo
echo "written to $work/"
