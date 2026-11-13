# Fixtures

## MOBI and AZW3

`lantern.html` is a two-paragraph story written for these tests, and
`lantern-cover.jpg` a 120×180 cover drawn for them with Pillow. Both are
dedicated to the public domain (CC0 1.0), and so is everything made from them.

`lantern.mobi` (MOBI 6) and `lantern.azw3` (KF8) were made from them with
calibre 9.14:

```sh
common=(--title "A Lantern in the Orchard" --authors "Ada Quill & Tomás Reed"
  --publisher "Orchard Press" --isbn 9780306406157 --tags "Fiction,Villages"
  --language en --pubdate 2001-04-05 --comments "A short story written for testing."
  --cover lantern-cover.jpg)
ebook-convert lantern.html lantern.mobi --mobi-file-type old "${common[@]}"
ebook-convert lantern.html lantern.azw3 "${common[@]}"
```

The other three are `lantern.mobi` with one thing broken. `r0` is the first
record's offset, the big-endian 32-bit number at byte 78 (136 here).

| File | Change |
|---|---|
| `lantern-no-exth.mobi` | The EXTH flag (`0x40`) cleared in the MOBI header's flags, at `r0 + 16 + 112` |
| `lantern-drm.mobi` | The PalmDOC encryption type, at `r0 + 12`, set to 2 |
| `lantern-truncated.mobi` | The first 1024 bytes only |
