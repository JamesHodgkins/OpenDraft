# Using OpenDraft with an AI assistant (MCP)

OpenDraft can let an AI assistant that speaks the [Model Context Protocol](https://modelcontextprotocol.io) look at your open drawings, measure and draw in them, and help with files. It is **off by default**, **read-only until you say otherwise**, and everything an assistant changes can be undone with one Undo.

> This is experimental. It has been tested on Windows. Linux compiles but the connection has not been run there yet.

## How it fits together

```
your assistant  <--MCP over stdio-->  opendraft-mcp  <--local socket-->  OpenDraft
```

`opendraft-mcp` is a small program that ships next to OpenDraft. Your assistant starts it. It finds the running OpenDraft through a file in your user profile and talks to it over a local socket that only your user account can reach. Nothing listens on the network.

OpenDraft does all the work and applies every change itself, through the same undo history your own edits use. The assistant never gets direct access to a drawing or to your disk.

## Set it up

1. **Get both programs.** A release has `opendraft` and `opendraft-mcp` in the same folder. From source: `cargo build --release -p od-app -p od-mcp` (both end up in `target/release`). If `opendraft-mcp` is somewhere else, set `OD_MCP` to its path before starting OpenDraft so Settings shows the right configuration.
2. **Turn it on.** In OpenDraft open Settings (type `OP` on the command line), go to **Automation**, tick **Let AI assistants work with the open drawings**, and press OK. The page shows the path of `opendraft-mcp` and a **Copy MCP config** button.
3. **Tell your assistant about it** (next section), then restart the assistant.
4. **Choose what it may do.** When the assistant connects it appears under **Connections** with three switches, all off:

| Switch | Lets it |
|---|---|
| **Allow changes** | add, change and delete entities, edit layers, set the selection, and undo or redo its own steps |
| **Allow commands** | run view commands (zoom, grid, snap, select all) and *start* draw or modify commands for you to finish |
| **Allow files** | open, save and export files, but only in the **Approved folders** below it |

Ticking or unticking takes effect on the assistant's next request. You can try things with none ticked: it can still read.

5. **Check it.** Run `opendraft-mcp --check`. It says whether it can reach OpenDraft and what it would be allowed to do.

Try asking: *"What drawings are open and what is in the active one?"*, then, with Allow changes ticked, *"Draw a 4 by 3 metre rectangle on a new layer called Walls."*

## Connecting an assistant

All of these start `opendraft-mcp` with no arguments. Use the path shown on the Automation page (the **Copy MCP config** button puts a ready-made entry on your clipboard). Settings locations change between versions of these programs, so check your assistant's own documentation if one of these does not match.

**Claude Desktop** — edit `claude_desktop_config.json` (Settings, Developer, Edit Config; on Windows it is in `%APPDATA%\Claude`):

```json
{
  "mcpServers": {
    "opendraft": { "command": "C:\\Program Files\\OpenDraft\\opendraft-mcp.exe" }
  }
}
```

**Claude Code**:

```bash
claude mcp add opendraft -- "C:\Program Files\OpenDraft\opendraft-mcp.exe"
```

**Cursor** — `.cursor/mcp.json` in your home or project folder uses the same `mcpServers` entry as Claude Desktop.

**VS Code** — `.vscode/mcp.json`:

```json
{
  "servers": {
    "opendraft": { "type": "stdio", "command": "C:\\Program Files\\OpenDraft\\opendraft-mcp.exe" }
  }
}
```

If more than one OpenDraft is running, add `"args": ["--instance", "<id>"]` with an id from `opendraft-mcp --check`.

## What an assistant can do

| Tool | What it does | Needs |
|---|---|---|
| `opendraft_instances_list` | lists running OpenDraft instances | nothing |
| `opendraft_documents_list`, `_document_get` | open drawings; one drawing's counts, units, bounds, layers, undo step | nothing |
| `opendraft_entities_query`, `_entities_get` | pages of entities (by kind, layer, space, window) or specific ones | nothing |
| `opendraft_layers_list`, `_selection_get` | layers; what is selected | nothing |
| `opendraft_geometry_measure` | exact length, area, centre, radius, end points | nothing |
| `opendraft_geometry_intersections` | where entities cross | nothing |
| `opendraft_drawing_summary` | an overview without reading every entity: counts by kind and layer, block names, the text on the drawing, the box round it all and a grid of where the objects are | nothing |
| `opendraft_drawing_audit` | housekeeping checks that change nothing: duplicates, overlaps, zero-length objects, empty text, unused blocks, layers and styles, and layers that are not on a standard you give | nothing |
| `opendraft_view_render` | a picture of a window, some entities or a layout sheet, as an SVG or (so the assistant can actually look at it) a PNG | nothing |
| `opendraft_calculate` | exact arithmetic and trigonometry for dimensions: named steps, each an expression that may use the steps before it (numbers, `+ - * / ^`, brackets, `pi`, `tau`, `e`, and functions such as `sin`, `atan2`, `sqrt`, `hypot`, `rad`, `deg`); a division by zero or the root of a negative is refused with the step named; changes nothing | none beyond reading |
| `opendraft_commands_list` | which commands automation may run or start | nothing |
| `opendraft_events_poll` | what changed in the app since the last look (drawings, commands, the selection) | nothing |
| `opendraft_operations_get`, `_operations_cancel` | watch (optionally waiting up to 30 seconds for the end) or stop a file operation, started command or selection request of its own | nothing |
| `opendraft_entities_apply_batch` | add, patch, replace and delete lines, circles, arcs, rectangles, polylines, ellipses and text, and add hatches (filled from closed shapes by id or added just before, with a named pattern or solid), as one undo step | Allow changes |
| `opendraft_entities_construct` | exact drafting operations on shapes, as one undo step: a line, a regular polygon, a tangent arc (a bend that leaves the end of a line or arc without a corner), move, rotate, scale, mirror, offset, rectangular and polar arrays (each optionally making copies); you say what to do and to what, it computes the coordinates | Allow changes |
| `opendraft_layers_apply_batch` | create layers, change them, set the current one | Allow changes |
| `opendraft_selection_set` | change what is selected | Allow changes |
| `opendraft_selection_request` | **ask you** to pick objects in the app and get their ids back | Allow changes |
| `opendraft_history_undo`, `_history_redo` | undo or redo a step it made | Allow changes |
| `opendraft_commands_invoke` | run zoom, grid, snap, select all | Allow commands |
| `opendraft_commands_start_interactive` | start Line, Move, Trim and so on **for you to finish** in the app | Allow commands |
| `opendraft_files_open`, `_files_save`, `_files_save_as`, `_files_export` | open, save, DXF or PDF/SVG export | Allow files |

Splines, block references, dimensions and hatches can be read and measured but not written yet.

**Looking without changing anything.** `opendraft_drawing_audit` finds things a person would tidy up with OVERKILL and PURGE, but only reports them: it never deletes or changes anything, and it also sees objects on locked layers. It can look at one layer, one space or one window at a time, which a very large drawing needs (more than 20,000 objects at once are not compared for duplicates and overlaps; the answer says so and how to narrow it). To check layer names, the assistant passes the names your standard allows (for example `A-WALL-*`); layer 0 and Defpoints always pass. Unused linetypes are not reported, because a new drawing carries the standard set. Names and text in the results come from the drawing and are passed to the assistant as data.

## Staying in control

- **One undo step per change.** A batch of edits is a single step named by the assistant. Press Ctrl+Z as usual. An assistant can only undo its *own* steps, and only while it is the top of the history, so it can never undo something you did.
- **Stale edits are refused.** An edit says which version of the drawing it was based on. If you changed the drawing in the meantime it is refused and the assistant has to look again.
- **It never talks over you.** Edits are refused while one of your commands is running or while a block is open in the Block Editor. Running or starting a command is also refused while a dialog is open.
- **It asks, you point.** When an assistant needs you to pick objects (`opendraft_selection_request`), a prompt labelled **ASSISTANT** appears in the command line, beside the cursor and in the status bar, with what it wants, for example `Pick the circle (exactly 1)`. Pick as you always do (click, window, crossing) and press Enter, or press Esc to refuse. The assistant gets only the objects you picked, and only those it asked for: if it asked for circles and you also picked a line, the line is taken out of the selection and you are told. It starts with an empty selection unless it asks to keep yours, never replaces a command you are running, is refused while a dialog is open, and is dropped if you switch to another drawing, start another command, untick **Allow changes**, or the assistant disconnects. If you do not finish within five minutes (the assistant may ask for up to fifteen) it ends as timed out. Nothing in the drawing changes.
- **Commands that need points stay yours.** When an assistant starts a command, its questions appear in OpenDraft and only you can answer them. It can watch what is being asked and cancel it.
- **Files stay in approved folders.** Add folders under **Approved folders** (the list starts empty). Paths with `..`, network paths, device names, links and unexpected file types are refused. Saving over the file a drawing already belongs to is allowed; **replacing any other existing file asks you first** in a "Replace file?" window that the assistant cannot answer.
- **Drawing text is data.** Text, layer names and block names in a drawing can say anything. They are passed to the assistant as data and never change what it is allowed to do.
- **What leaves your computer.** OpenDraft itself sends nothing anywhere. Whatever the assistant reads is sent to the service your assistant uses, so only turn this on for drawings you are happy to share with it. The file tools show the assistant file *names*, not folders.

## Retrying safely

Every tool that changes something (edits, undo and redo, running or starting a command, asking you to select, opening, saving and exporting) accepts an optional `idempotency_key`: any unique string of up to 64 letters, digits and `_ - . :`. If a request with a key is sent again with the same arguments, nothing is done twice. The first answer comes back instead, with `"replayed": true`. This is what lets an assistant safely try again after a timeout without drawing a line twice or flipping a setting back.

- The same key with *different* arguments is refused (`idempotency_key_reused`): use a new key for a new request.
- Only successes are remembered. A request that failed changed nothing, so asking again just runs it.
- A key is remembered per connection and per tool, and only the last 256 are kept; an older key simply runs again.
- If the connection drops while a change is being made, the program does **not** retry it on its own (the reply may be what was lost). It reports `instance_gone` with `outcome_unknown`, and the assistant should read the drawing's revision, or repeat the request with the same key.

## Stopping and revoking

- Untick a switch to take that permission back at once.
- Untick **Let AI assistants work with the open drawings** (or close OpenDraft) to disconnect everything. The connection file is removed when OpenDraft closes normally.
- Setting the environment variable `OD_AUTOMATION=1` turns the server on for that run only, without saving it. It never grants any of the switches.

## When something does not work

Run `opendraft-mcp --check` first.

| You see | What it means |
|---|---|
| "No OpenDraft is offering a connection" / `app_not_running` | OpenDraft is not running, or Settings > Automation is not ticked |
| "not running: a leftover" | a crash left its connection file behind; it is removed the next time OpenDraft starts |
| "different automation API versions" | `opendraft-mcp` and OpenDraft are from different releases: use both from the same one |
| `instance_required` | several OpenDrafts are running: add `--instance <id>` |
| `scope_denied` | the matching switch is off for this connection |
| `stale_revision` | the drawing changed since it was read: look again, then retry |
| `command_busy` | a command is running in OpenDraft: finish or cancel it |
| `operation_busy` | a dialog is open (for commands and for asking you to select), or too many operations are waiting |
| `deadline_exceeded` on a selection request | you did not finish selecting in time: the assistant should ask again only if you still want it |
| `block_editor_open` | close the Block Editor (BCLOSE) |
| `path_denied` | the file is outside the approved folders, or not an allowed kind of path |
| `consent_required` | you declined (or did not answer) the "Replace file?" question |
| `result_too_large` | ask for less: a smaller window, fewer entities |
| `idempotency_key_reused` | the key was used for a different request: use a new key |
| `instance_gone` with `outcome_unknown` | the connection dropped during a change: read the drawing before retrying, or retry with the same `idempotency_key` |
| the assistant lists no tools | restart the assistant after changing its MCP settings; check the path in the config exists |

If `--check` can connect but the assistant cannot, the assistant's MCP settings are the problem. Make sure the `command` is the full path and that it points at `opendraft-mcp`.

## Versions

| Part | Version | Notes |
|---|---|---|
| OpenDraft and `opendraft-mcp` | 0.1.x | ship together and should be upgraded together |
| Automation API (app to program) | 1 | negotiated when the program connects; a different major version is refused with a message |
| MCP protocol (assistant to program) | 2025-11-25 and 2026-07-28 | both tested. The SDK also knows 2024-11-05, 2025-03-26 and 2025-06-18; those are not tested here |

Tool names are part of the interface and will not be repurposed. New optional fields can appear in results.

## Limits

Per request: 1,000 operations in a batch, 1,000 entities per page, 200 entities for intersections, 5,000 entities and 500,000 points in a picture, 6 MiB for a picture (a PNG is at most 4096 pixels on a side and 4 million pixels), 64 KiB per text, 8 operations in flight per connection; an audit compares at most 20,000 objects (and a million pairs) for duplicates and overlaps, and lists 30 findings by default and 200 at most; a request to select takes a prompt of up to 200 characters and up to 1,000 objects, and one wait is at most 30 seconds. Reading a 100-entity page of a 10,000-object drawing takes about a millisecond; the connection program idles at about 7 MiB.

## Not available yet

Streamable HTTP (remote use), resource and event subscriptions (events are polled; pan and zoom are not events), asking you for typed numbers or choices (only for objects so far), writing splines, blocks, dimensions and hatches, and a log of what assistants did. Linux is untested for this feature.
