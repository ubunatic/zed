# Plan: Image paste into editor (e.g. Markdown files)

## Problem

`Editor::paste` in `crates/editor/src/editor.rs:14777` only handles `ClipboardEntry::String`.
When the clipboard contains only a `ClipboardEntry::Image` or `ClipboardEntry::ExternalPaths`
pointing to an image file, the paste silently inserts nothing.

The agent chat editor (`crates/agent_ui/src/message_editor.rs`) already handles image paste,
but using AI-specific machinery (creases, mention sets, language model image encoding). That
code is not reusable as-is for a plain file editor — we need a simpler, file-oriented approach.

---

## Design decision: what to do with the image bytes

When a user pastes an image into a Markdown file, the natural expectation is:
1. The image bytes are saved as a file next to (or under) the current document.
2. A Markdown image link `![](relative/path.png)` is inserted at the cursor.

For non-Markdown buffers (plain text, unknown language) we can either:
- Do nothing (safest default for v1).
- Insert an absolute file path link.

**v1 scope:** Markdown only. Save to `<document-dir>/assets/<uuid>.png` (or detected format),
insert `![](assets/<uuid>.png)` at cursor.

---

## What to reuse / not reuse from `agent_ui`

| `agent_ui` component | Verdict |
|---|---|
| `load_external_image_from_path` | **Copy or extract** — pure utility, reads bytes + guesses format. Move to `util` or duplicate in `editor`. |
| `image_format_from_external_content` | **Copy** with `load_external_image_from_path`. |
| `is_raster_image_path` | **Copy** — needed to filter `ExternalPaths` entries. |
| `insert_images_as_context` / creases / mention_set | **Do not reuse** — AI-specific, heavy deps. |
| `resolve_pasted_context_items` | **Inspire from** — the entry-dispatch logic (Image vs ExternalPaths) is the right pattern. |
| `handle_pasted_context` / `paste_images_as_context` | **Inspire from** — entry-priority guard (`String` first = skip) is correct. |

**Extraction strategy:** move `load_external_image_from_path`, `image_format_from_external_content`,
and `is_raster_image_path` into `crates/util/src/image_util.rs` (new file) and depend on them
from both `agent_ui` and `editor`. Avoids duplication without introducing circular deps.

---

## Implementation steps

### Step 0 – Extract shared image utilities
- Create `crates/util/src/image_util.rs` with:
  - `load_external_image_from_path(path, default_name) -> Option<(gpui::Image, SharedString)>`
  - `image_format_from_external_content(fmt) -> Option<gpui::ImageFormat>`
  - `is_raster_image_path(path) -> bool`
- Re-export from `crates/util/src/lib.rs`.
- Update `agent_ui` to use the shared versions.

### Step 1 – Add `paste_image` to `Editor`

In `crates/editor/src/editor.rs`, add a helper:

```rust
fn paste_image(
    &mut self,
    image: gpui::Image,
    image_name: SharedString,
    window: &mut Window,
    cx: &mut Context<Self>,
)
```

Logic:
1. Detect buffer language. If not Markdown, return early (no-op for v1).
2. Determine the document's directory from the buffer's file path. If no path (unsaved buffer),
   return early.
3. Build `assets/` subdirectory path next to the document.
4. Generate a unique filename: `<sanitized_name_or_uuid>.<ext>` where ext comes from
   `image.format()`.
5. Spawn a background task to write the bytes to disk via `std::fs::write`.
6. On success (back on foreground), insert `![<image_name>](assets/<filename>)` at each cursor
   using the existing `do_paste` / `insert` path.
7. On failure, log the error (`.log_err()`).

### Step 2 – Wire image paste into `Editor::paste`

Extend the match in `paste` (line 14782–14796):

```rust
pub fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
    ...
    if let Some(item) = cx.read_from_clipboard() {
        // Priority 1: string entry (existing behavior)
        let clipboard_string = item.entries().iter().find_map(|entry| match entry {
            ClipboardEntry::String(s) => Some(s),
            _ => None,
        });
        if let Some(s) = clipboard_string {
            self.do_paste(s.text(), s.metadata_json(), true, window, cx);
            return;
        }
        // Priority 2: image entry
        if let Some(image_entry) = item.entries().iter().find_map(|entry| match entry {
            ClipboardEntry::Image(img) => Some(img.clone()),
            _ => None,
        }) {
            self.paste_image(image_entry, "Image".into(), window, cx);
            return;
        }
        // Priority 3: external file paths that are images
        if let Some(paths) = item.entries().iter().find_map(|entry| match entry {
            ClipboardEntry::ExternalPaths(p) => Some(p.paths().to_owned()),
            _ => None,
        }) {
            for path in paths {
                if util::is_raster_image_path(&path) {
                    if let Some((img, name)) =
                        util::load_external_image_from_path(&path, &"Image".into())
                    {
                        self.paste_image(img, name, window, cx);
                    }
                }
            }
            return;
        }
        // Fallback: paste whatever text is available
        self.do_paste(&item.text().unwrap_or_default(), None, true, window, cx);
    }
}
```

### Step 3 – Tests (write first)

Add to `crates/editor/src/editor_tests.rs`, after the existing markdown paste tests (~line 32470):

#### `test_paste_image_into_markdown_inserts_link`
- Set up an `EditorTestContext` with a Markdown buffer backed by a temp file on disk.
- Write a `ClipboardItem` containing a `ClipboardEntry::Image` (a 1×1 PNG).
- Call `paste`.
- Assert the buffer text contains `![Image](assets/` and ends with `.png)`.
- Assert the image file was written to the `assets/` subdirectory.

#### `test_paste_image_path_into_markdown_inserts_link`
- Same but the clipboard contains `ClipboardEntry::ExternalPaths` pointing to a PNG file
  in a temp dir.
- Assert the markdown link uses the source filename.

#### `test_paste_image_into_non_markdown_is_noop`
- Set up a Rust buffer.
- Write an image clipboard entry.
- Call `paste`.
- Assert the buffer is unchanged.

#### `test_paste_image_into_unsaved_buffer_is_noop`
- Set up a Markdown buffer with no file path.
- Write an image clipboard entry.
- Call `paste`.
- Assert the buffer is unchanged (no panic, no crash).

#### `test_paste_text_takes_priority_over_image`
- Clipboard has both `String` and `Image` entries.
- Assert only the text is pasted (existing behavior preserved).

#### Manual Test Section
Copy+paste images here:

---

## File changes summary

| File | Change |
|---|---|
| `crates/util/src/image_util.rs` | New — shared image utilities |
| `crates/util/src/lib.rs` | Add `pub mod image_util; pub use image_util::*;` |
| `crates/util/Cargo.toml` | Add `image` crate dependency (same version as `agent_ui`) |
| `crates/agent_ui/src/mention_set.rs` | Replace local helpers with `util::*` imports |
| `crates/editor/src/editor.rs` | Add `paste_image`, extend `paste` |
| `crates/editor/src/editor_tests.rs` | Add 5 new tests (listed above) |
| `crates/editor/Cargo.toml` | Add `image` dep if not already present |

---

## Open questions / future work

- Multi-cursor image paste: insert one image per cursor or one shared image? (v1: single insert
  at newest cursor, same as text paste fallback)
- Remote/SSH projects: `std::fs::write` won't work. Detect `project.is_local()` and skip for v1.
- Settings: should image paste be opt-in? Skip for v1, add setting later if needed.
- Other languages: HTML `<img>` tag, RST `.. image::`, AsciiDoc `image::` — out of scope for v1.
