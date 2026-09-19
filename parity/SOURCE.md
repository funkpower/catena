# What the golden files were generated from

The oracles import the Python these tools were ported from. The golden files
in this directory are the answers that Python gave, at one commit:

| Source | Commit |
|---|---|
| `book-pipeline` | `7499e3257572968b437ecfdc79feccc91b96585b` |
| `podcast-download-systemd` | `ab20139cd0defb400a0dd9ed9034e03c1d1e356d` |
| `podcast-pipeline` | `4566302444a6aa3609bd0d6685323619bd33c72b` |
| `knowledge-mcp` | `3d02cd5f9985e09f7354178e1907dbc28a7436da` |

Regenerating against a different commit will move the goldens. That is the
point — the diff is the review. Record the new commit here in the same change.

The oracles find the source through two environment variables, because where
a checkout lives is a fact about one machine and must not be compiled in:

```sh
CATENA_PARITY_PYTHON=/path/to/venv/bin/python
CATENA_PARITY_PYTHONPATH=/path/to/book-pipeline/src:/path/to/podcast-pipeline/src:/path/to/knowledge-mcp/src
CATENA_PARITY_NODE=node   # only if it is not already on PATH
```

Two oracles are JavaScript rather than Python, because the code they compare
against is. Their functions are copied into `oracle/cuenorm.mjs` and
`oracle/compatible.mjs` verbatim, since the modules they live in open a
database connection when they load.
