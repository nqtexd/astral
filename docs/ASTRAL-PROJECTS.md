# Projects and customization

Create from the library or open an .astral file. Save/Save As preserves editable data and linked assets subject to conflict checks. Favorite projects, search titles/paths and sort by name/modification date. Refresh updates cached metadata. Edited and opened times are separate.

## Rename, changes and Trash

A card's options menu offers Rename, Recent changes and Move to Trash. Rename changes the title and saves; filenames and asset paths stay stable. It is undoable within the open session.

Trash saves current edits and moves only the project file into `.astral-trash` beside the original. It closes the document. Linked assets remain because they may be shared. Restore refuses an existing destination or open copy. There is no automatic permanent deletion.

Recent changes shows up to 200 local entries: time, revision, Edit/Undo/Redo and changed sections. Its sidecar lives in `.astral-history` beside the saved project. Save As carries the log. This is a bounded activity record, not document snapshots, a content diff or multi-user version control.

MCP exposes `project_activity`, `rename_project`, `trash_project` and `restore_project`. Mutations require the current revision. Rename via MCP changes the title; call `save_project` afterward. Trash saves/closes. Restore accepts a relative configured-workspace trash path. External locations chosen through the desktop remain confined to their authorized roots.

## Appearance

Settings → Appearance contains accent presets/custom accent, canvas surround, compact controls, stronger contrast, scale, fit-to-window and corners. Workspace options control panels, pro controls, editor-only alignment grid, frame-card width and recovery interval (5–120 seconds). New-project dimensions/FPS do not change existing documents. Reset appearance restores appearance/panel defaults without deleting projects.

Drawing settings include independent single-frame drawing, onion range/opacity, suggestions and model choice. Suggestions stay ghosts until accepted. Voice uses installed Windows voices. Agents provides a ready-to-copy local MCP configuration.

The Video encoder section selects a separately installed FFmpeg. The public ZIP omits encoder binaries. PNG previews work without an encoder; video/media conversion and assembly need one.
