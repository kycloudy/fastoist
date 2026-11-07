# Fastoist

A fast, native Todoist client for Windows. One 9 MB `.exe`, no installer, no browser engine. It syncs with your real Todoist account through the Todoist API.

## Get it

Download `fastoist.exe` from [Releases](../../releases). Put it anywhere and run it.

## Set up

1. Copy your API token from Todoist: **Settings > Integrations > Developer** ([direct link](https://app.todoist.com/app/settings/integrations/developer)).
2. Paste it into Fastoist and click **Connect**.
3. Optional: to show calendar events (classes, appointments), open **Settings** (gear icon) and paste an iCal URL, one per line. For Google Calendar, use **Calendar settings > Integrate calendar > Secret address in iCal format**. A local `.ics` file path also works.

## Views

- **Inbox**, **projects**, and **sections**, with nested sub-tasks.
- **Today**: overdue tasks, today's tasks grouped by project or priority, and today's events.
- **Upcoming**: month, week (hour grid), and list layouts. The **Plan** panel lists all-day tasks for the next 7 days and tasks that have a deadline but no date. Completed tasks show crossed out.
- **Filters & Labels**: your saved filters run on Todoist's servers, so every filter query works exactly as in the official app.
- **Search** across tasks, projects, filters, and labels.

## Working with tasks

| Action | How |
| --- | --- |
| Add a task | `Q`, or **Add task**. The name understands quick-add syntax: `Essay fri 3pm #POL620 @exam p1`. |
| Complete | Click the circle. `Ctrl+Z` or **Undo** reverses it. |
| Edit | Click the task. `Ctrl+Enter` saves. |
| Reschedule | Drag a task onto a day, a time slot in week view, or a day in list view. Right-click for Today, Tomorrow, Next week, No date, and priority. |
| Add on a day | Double-click a day in month view. |
| Search | `/`, `F`, or `Ctrl+K` |
| Sync now | `Ctrl+R` (Fastoist also syncs every 60 seconds) |
| Toggle sidebar | `M` |
| Back / forward | `Alt+Left` / `Alt+Right` |

## System tray

Fastoist puts an icon in the Windows notification area. Closing the window sends the app to the tray, so it keeps syncing. Left-click the icon to show the window. Right-click it for the menu:

- **Add task** (`Ctrl+Space`) and **Show Fastoist** (`Win+Alt+S`). Both work from any app. If another program already uses a hotkey, the menu item still works.
- **Today** and **Tomorrow**: your tasks and your calendar events (from the iCal feeds in Settings), in time order. Overdue tasks appear under Today. Click a task to open it.
- **Inbox**, **Today**, **Upcoming**, **Filters and Labels**, **Completed**, and a **Favorites** submenu.
- **Settings**, **Stay running in tray** (turn it off and closing the window quits), and **Quit Fastoist**.

Starting Fastoist a second time shows the running window instead of opening another copy.

## Data

Fastoist keeps its settings and an offline copy of your account in `%LOCALAPPDATA%\Fastoist`. The copy makes startup instant. The API token is stored in `config.json` in that folder, readable by your Windows user account. You can also set the `TODOIST_TOKEN` environment variable instead.

## Build from source

```
cargo build --release
```

To cross-compile from Linux: install `gcc-mingw-w64-x86-64`, run `rustup target add x86_64-pc-windows-gnu`, then `cargo build --release --target x86_64-pc-windows-gnu`.

## Known limits

- Not yet supported: comments, reminders, attachments, collaborators and assignments, drag to reorder within a list, board layout, and creating or editing projects, sections, labels, and filters.
- iCal times with a time zone are read as your local time. This is correct when the calendar uses your own time zone.
