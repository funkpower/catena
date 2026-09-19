# emb

Lines into vectors, against an embedding server.

Back to [the index](README.md).

## What it does

Sends each line of its input to an HTTP embedding endpoint and writes the
vector that comes back, one JSON array per line, in the order the lines
arrived.

It never loads a model. An embedding model is half a gigabyte of weights and a
GPU, which belongs behind a server where it can be shared — what this owns is
everything around the call: batching, a defensive truncation, a retry, and the
assertion that the vectors are the width the index expects.

## When you would use it

- **At the end of an ingest pipeline**, where passages become rows.
- **Re-embedding an existing corpus** after changing model, where the thing
  that matters is that every vector is the same width as the last.
- **Anywhere a GPU is somebody else's problem.** One server, many machines
  running this.

## Install

```sh
cargo install --git https://git.pumpin.online/funkpower/catena emb
```

## Example

```
$ emb --host http://127.0.0.1:11434 --model bge-m3 --dim 4 lines.txt
[0.0123,-0.0456,0.0789,0.0012]
[0.0123,-0.0456,0.0789,0.0012]
```

Asking for a width the server does not produce is refused, rather than
half-filling an index:

```
$ emb --host http://127.0.0.1:11434 --model bge-m3 --dim 1024 lines.txt
emb: vectors are 4 wide, expected 1024
$ echo $?
65
```

At the end of a pipeline:

```sh
mdscrub chapter.md | chunkmd | jq -r .text \
  | emb --model bge-m3 --dim 1024 > vectors.ndjson
```

Because the output lines up with the input line for line, the two can be put
back together:

```sh
paste -d'\t' <(chunkmd book.md | jq -c .) vectors.ndjson
```

## Options

| Option | Default | Effect |
|---|---|---|
| `--host <URL>` | `http://localhost:11434` | The server to ask |
| `--model <NAME>` | required | The model to ask for |
| `--dim <WIDTH>` | — | Refuse vectors that are not this wide |
| `--batch <COUNT>` | `32` | Texts per request |
| `--max-input-tokens <TOKENS>` | `1800` | Cut each text to this many tokens before sending it; `0` sends them whole |
| `--timeout <SECONDS>` | `600` | How long to wait for one request |
| `--attempts <COUNT>` | `3` | How many times to try a failing request |

The input cap is a defensive one, not a model limit. A server returns an error
for an input past its context length, and that error fails the whole batch —
so one oversized passage costs thirty-one good ones.

The timeout is generous because a cold server loading a model onto a GPU can
take minutes before it answers at all.

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Every line was embedded |
| `64` | The command line was wrong |
| `65` | The server answered something that was not an embedding, or was the wrong width |
| `69` | The server could not be reached |
| `74` | The input could not be read |

A width mismatch and a missing vector are refused immediately and not retried:
the server answered, it just did not answer the question.

## The request

`POST {host}/api/embed`, with:

```json
{"model": "bge-m3", "input": ["the mill stood", "at the northern end"]}
```

and expecting:

```json
{"embeddings": [[0.0123, -0.0456, …], [0.0021, 0.0310, …]]}
```

which is the shape Ollama uses and several other servers speak.

## Empty lines

Sent as they are. Dropping them would make the output stop lining up with the
input, which is the one property that makes this composable.

## As a library

```rust
let endpoint = emb::Endpoint { dim: Some(1024), ..emb::Endpoint::new(host, "bge-m3") };
let agent = ureq::Agent::new_with_defaults();
let vectors = emb::embed_batch(&agent, &endpoint, &texts)?;
```

## See also

- [`chunkmd`](chunkmd.md) and [`cuepack`](cuepack.md) — where the text comes
  from.
- [`rrf`](rrf.md) — what to do with the results of searching those vectors.
