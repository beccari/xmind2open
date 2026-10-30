# xmind2open

Command-line tool that converts XMind workbooks (`.xmind`) to open formats.

It reads both XMind Zen / 2020+ workbooks (`content.json`) and XMind 8
workbooks (`content.xml`). By default it writes a single **OPML** file, the
most widely supported outline interchange format (Freeplane, MindNode,
iThoughts, WorkFlowy, Dynalist and XMind itself can import it). Other formats
are opt-in with `-f`.

## Install

Requires a Rust toolchain ([rustup.rs](https://rustup.rs)).

```sh
cargo install --path .     # installs `xmind2open` into ~/.cargo/bin
# or just build it:
cargo build --release      # binary at target/release/xmind2open
```

## Usage

```sh
xmind2open plan.xmind                     # writes ./plan.opml
xmind2open plan.xmind -f md               # Markdown outline instead
xmind2open plan.xmind -f mm -f html       # FreeMind maps + HTML viewer
xmind2open plan.xmind -f mm,png -o out    # comma-separated works too
xmind2open plan.xmind -f all -o out/plan  # every format
```

Options:

| Option                | Description                                               |
| --------------------- | --------------------------------------------------------- |
| `-f, --format <FMT>`  | Output format; repeat or comma-separate. Default: `opml`  |
| `-o, --output <DIR>`  | Directory to write into (created if missing). Default: `.` |
| `-q, --quiet`         | Only print errors                                         |
| `-h, --help`          | Full help                                                 |

## Formats

| Format    | File                    | Scope          | Notes                                    |
| --------- | ----------------------- | -------------- | ---------------------------------------- |
| `opml`    | `<name>.opml`           | all sheets     | **default**                              |
| `md`      | `<name>.md`             | all sheets     | nested bullets, notes as blockquotes     |
| `mm`      | `<name>[ - NN-sheet].mm`   | one per sheet  | FreeMind / Freeplane                     |
| `mermaid` | `<name>[ - NN-sheet].mmd`  | one per sheet  | `graph TD`                               |
| `dot`     | `<name>.dot`            | all sheets     | Graphviz                                 |
| `html`    | `<name>.html`           | all sheets     | self-contained, collapsible, zoomable    |
| `png`     | `<name>[ - NN-sheet].png`  | one per sheet  | needs Graphviz `dot`                     |
| `jpg`     | `<name>[ - NN-sheet].jpg`  | one per sheet  | needs `dot` and macOS `sips` or ImageMagick |

The `- NN-sheet` suffix is only added when the workbook has more than one sheet.

### How XMind-only concepts are mapped

Open formats have no native "range of siblings" grouping, so:

- **Summaries** become an extra child prefixed `Σ` on the parent topic, with a
  `covers` list of the siblings they span.
- **Boundaries** become an extra child labelled `Group`, with the same `covers` list.
- **Notes** and **markers** are kept (OPML `_note` / `_markers`, FreeMind
  rich-content notes, Markdown blockquotes, note nodes in Mermaid / DOT).
