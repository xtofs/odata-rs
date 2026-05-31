# Zero-copy token strings

`CsdlToken` uses `Cow<'a, str>` for `name` and attribute values, but the current `CsdlReader::next_token` always stores tokens through the owned `OwnedToken` slot (`self.held`) and returns `Cow::Borrowed` references into that owned data. This means every element token incurs at least one `String` allocation per name and one per attribute value.

The original design intent was: tokens should borrow zero-copy from `self.buf` on the hot path.

The blocker was lifetime variance — `BytesStart<'b>` borrows `self.buf` with a lifetime tied to the `read_event_into` call, and the borrow checker won't extend that to the `'_` of the returned token while also allowing us to mutate `self.deferred` / `self.held` in the same arm.

Options to investigate:
1. **Split the API into two paths**: one fast path that returns a borrowed token directly without touching deferred state (for plain Start/End/Empty of non-Annotation elements), and the current owned path only for the cases that genuinely need queueing.
2. **Use raw pointers internally**, with a clearly documented invariant that `self.buf` is not mutated while a returned token is alive. Unsafe but tractable.
3. **Restructure**: drive the reader via a visitor callback (`read(|token| ...)`) where the borrow lives inside the callback scope. Changes the API shape but resolves the lifetime issue cleanly.

Worth doing once the EdmModel builder is in place and we can measure the actual allocation overhead on real-world CSDL files.
