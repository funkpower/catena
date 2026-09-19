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
    chunk_markdown,
    count_tokens,
    fuse_hits,
    identify,
    pack_cues,
    parse_subtitles,
    scrub,
    series_of,
    split_collection,
)

__all__ = [
    "__version__",
    "chunk_markdown",
    "count_tokens",
    "fuse_hits",
    "identify",
    "pack_cues",
    "parse_subtitles",
    "scrub",
    "series_of",
    "split_collection",
]
