# Known divergences

Places where the Rust and the Python deliberately do not agree, each with the
reason it is acceptable. A divergence that is not written down here is a bug,
and loosening a test instead of adding an entry here is how a port rots.

Format:

```
## <function> — <one-line summary>

**Python:** what it does.
**Rust:** what it does instead.
**Why this is acceptable:** the argument, including what input class is
affected and how often it occurs in practice.
**Measured:** how many records out of how many differ.
```

---

None recorded yet.
