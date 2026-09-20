"""The catena libraries, in process.

Every function here is the same code the command-line tool runs, and hands
back the same record shapes it writes to stdout — dictionaries rather than
lines of JSON.

    >>> import catena
    >>> chunks = catena.chunk_markdown(markdown, target=512)
    >>> chunks[0]["page"]
    41
"""

from ._catena import (
    __version__,
    cue_windows,
    chunk_markdown,
    citation_prompt,
    count_tokens,
    find_citations,
    fuse_hits,
    identify,
    looks_like_footnote_citations,
    merge_citation_reply,
    pack_cues,
    parse_identifier,
    parse_subtitles,
    scrub,
    segment_cues,
    series_of,
    split_collection,
)

__all__ = [
    "__version__",
    "cue_windows",
    "chunk_markdown",
    "citation_prompt",
    "count_tokens",
    "find_citations",
    "fuse_hits",
    "identify",
    "looks_like_footnote_citations",
    "merge_citation_reply",
    "pack_cues",
    "parse_identifier",
    "parse_subtitles",
    "scrub",
    "segment_cues",
    "series_of",
    "split_collection",
]
