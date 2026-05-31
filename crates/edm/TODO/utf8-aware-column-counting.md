# UTF-8 aware column counting in Location

`reader::Location::column` currently counts **bytes** within a line, not Unicode characters. For ASCII CSDL — the common case — byte column equals character column. Annotation strings, namespaces, or any other content containing multi-byte UTF-8 (e.g. accented characters, CJK) make the column larger than what a text editor would report.

Where this lives: `ByteCounter::consume` in `src/reader/mod.rs`. The current loop is:

```rust
for &b in &buf[..n] {
    if b == b'\n' {
        self.line += 1;
        self.column = 1;
    } else {
        self.column += 1;
    }
}
```

To make column character-accurate while still working byte-by-byte (we don't have a `&str`), only advance `column` on **UTF-8 leading bytes** — i.e. bytes whose top two bits aren't `10`. Continuation bytes (`0b10xxxxxx`) belong to a multi-byte character that was already counted:

```rust
for &b in &buf[..n] {
    if b == b'\n' {
        self.line += 1;
        self.column = 1;
    } else if (b & 0b1100_0000) != 0b1000_0000 {
        // not a UTF-8 continuation byte
        self.column += 1;
    }
}
```

Caveat: this counts *code points*, not grapheme clusters. A combining accent or emoji ZWJ sequence still reports more columns than a human would point at. Good enough for error messages; full grapheme-accurate counting would need a Unicode segmentation crate and is almost certainly not worth it for CSDL diagnostics.
