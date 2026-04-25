zed
2026-04-26T01:08:38+02:00 INFO  [zed] ========== starting zed version 0.235.0+dev.be0e5d7c3bb3ef8d12c3a12c6e769cf59ab2c56a, sha be0e5d7 ==========
2026-04-26T01:08:38+02:00 INFO  [gpui_linux::linux::platform] Compositor GPU hint: vendor=0x1002, device=0x15bf (from dev 226:128)

thread 'main' (1814226) panicked at crates/zed/src/zed.rs:2097:87:
called `Result::unwrap()` on an `Err` value: Error loading built-in keymap "keymaps/default-linux.json": Errors in user keymap file.
In section with `context = "!Terminal"`:

- In binding `"ctrl-shift-c"`, didn't find an action named `"collab_panel::ToggleFocus"`.
In section with `context = "CollabPanel && not_editing"`:

- In binding `"ctrl-backspace"`, didn't find an action named `"collab_panel::Remove"`.
In section with `context = "CollabPanel"`:

- In binding `"alt-up"`, didn't find an action named `"collab_panel::MoveChannelUp"`.

- In binding `"alt-down"`, didn't find an action named `"collab_panel::MoveChannelDown"`.

- In binding `"alt-enter"`, didn't find an action named `"collab_panel::OpenSelectedChannelNotes"`.

- In binding `"shift-enter"`, didn't find an action named `"collab_panel::ToggleSelectedChannelFavorite"`.
In section with `context = "(CollabPanel && editing) > Editor"`:

- In binding `"space"`, didn't find an action named `"collab_panel::InsertSpace"`.
In section with `context = "ChannelModal"`:

- In binding `"tab"`, didn't find an action named `"channel_modal::ToggleMode"`.
In section with `context = "ChannelModal > Picker > Editor"`:

- In binding `"tab"`, didn't find an action named `"channel_modal::ToggleMode"`.
stack backtrace:
0: __rustc::rust_begin_unwind
1: core::panicking::panic_fmt
         2: core::result::unwrap_failed
            3: zed::zed::load_default_keymap
               4: zed::zed::handle_keymap_file_changes
                  5: zed::main::{closure#11}
                     6: <<gpui::app::Application>::run<zed::main::{closure#11}>::{closure#0} as core::ops::function::FnOnce<()>>::call_once::{shim:vtable#0}
                        7: <gpui_linux::linux::platform::LinuxPlatform<gpui_linux::linux::wayland::client::WaylandClient> as gpui::platform::Platform>::run
                           8: <gpui::app::Application>::run::<zed::main::{closure#11}>
                              9: zed::main
                              note: Some details are omitted, run with `RUST_BACKTRACE=full` for a verbose backtrace.
                              [1]    1814226 IOT instruction (core dumped)  zed
