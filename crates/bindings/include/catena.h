/*
 * catena — the libraries behind one surface, for C callers.
 *
 * Text goes in as NUL-terminated UTF-8; records come back as a
 * NUL-terminated JSON array, the same document the command line writes and
 * under the same stability promise. Release every returned string with
 * catena_string_free.
 *
 * A function that can fail returns NULL and leaves a message in
 * catena_last_error, which is thread-local.
 *
 * Every function is safe to call from any thread.
 *
 * Licence: MPL-2.0. See https://git.pumpin.online/funkpower/catena
 */
#ifndef CATENA_H
#define CATENA_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Bumped when the meaning or signature of anything here changes. Additions
 * leave it alone. Check it against CATENA_ABI_VERSION at load time. */
#define CATENA_ABI_VERSION 1u

unsigned int catena_abi_version(void);

/* Why the last call on this thread failed, or NULL if none did. Valid until
 * the next failing call on the same thread. Do not free. */
const char *catena_last_error(void);

/* Release a string this library returned. NULL is allowed. */
void catena_string_free(char *text);

/* Cut markdown into retrievable passages. Returns a JSON array of chunks.
 * Zero for a budget means that budget's default. */
char *catena_chunk_markdown(const char *markdown,
                            size_t target,
                            size_t overlap,
                            size_t min_tokens,
                            size_t hard_max);

/* Clean converted markdown. Non-zero `pages` normalises page markers as
 * well, which is not idempotent. Returns the cleaned text. */
char *catena_scrub(const char *markdown, int pages);

/* How many tokens a string is. Zero on error. */
size_t catena_count_tokens(const char *text);

/* Read any of the subtitle formats. Returns a JSON array of cues. */
char *catena_parse_subtitles(const char *content, int keep_repeats);

/* Pack cues into passages. `cues` is a JSON array of cues. Zero for either
 * budget means the default. Returns a JSON array of chunks. */
char *catena_pack_cues(const char *cues, double seconds, size_t max_tokens);

/* Split a collection into the works inside it. Returns a JSON array of
 * sections, or NULL when the document is one work rather than a collection —
 * an answer, not a failure; the reason is in catena_last_error. */
char *catena_split_collection(const char *markdown);

/* Fuse ranked lists into one. `hits` is a JSON array; `weights` a JSON object
 * of channel to weight, or NULL. Zero `k` means the default, zero `limit`
 * means all of them. Returns a JSON array. */
char *catena_fuse_hits(const char *hits,
                       double k,
                       const char *weights,
                       size_t limit);

/* Say what a file is from its first bytes and its name. Returns a JSON doc
 * record. */
char *catena_identify(const char *name, const uint8_t *data, size_t len);

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* CATENA_H */
