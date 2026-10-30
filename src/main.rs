mod emit;
mod html;
mod load;
mod model;
mod render;

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, ValueEnum};
use regex::Regex;

use model::{Counts, Sheet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Format {
    /// OPML 2.0 outline, one file with every sheet (the default)
    Opml,
    /// FreeMind / Freeplane mind map, one .mm per sheet
    Mm,
    /// Markdown nested bullet outline, one file with every sheet
    Md,
    /// Mermaid `graph TD` diagram, one .mmd per sheet
    Mermaid,
    /// Graphviz DOT, one file with every sheet
    Dot,
    /// Self-contained interactive HTML viewer
    Html,
    /// PNG image per sheet (needs Graphviz `dot`)
    Png,
    /// JPG image per sheet (needs Graphviz `dot` and macOS `sips` or ImageMagick)
    Jpg,
    /// Every format above
    All,
}

const ALL: [Format; 8] = [
    Format::Opml,
    Format::Mm,
    Format::Md,
    Format::Mermaid,
    Format::Dot,
    Format::Html,
    Format::Png,
    Format::Jpg,
];

/// Convert an XMind workbook (.xmind) to an open format.
///
/// Reads both XMind Zen/2020+ (content.json) and XMind 8 (content.xml)
/// workbooks. By default only OPML is written, the most widely supported
/// outline interchange format (Freeplane, MindNode, iThoughts, WorkFlowy,
/// Dynalist and XMind itself can import it).
///
/// OPML, Markdown, DOT and HTML hold every sheet in one file. FreeMind,
/// Mermaid, PNG and JPG are written once per sheet, named
/// "<name> - 01-<sheet>.<ext>" when the workbook has more than one sheet.
///
/// XMind summaries are written as an extra "Σ" child on the parent topic that
/// lists the siblings it covers; boundaries are written as "Group".
#[derive(Parser)]
#[command(version, after_help = "\
EXAMPLES:
  xmind2open plan.xmind                     write plan.opml in the current directory
  xmind2open plan.xmind -f md               write a Markdown outline instead
  xmind2open plan.xmind -f mm -f html       FreeMind maps plus an HTML viewer
  xmind2open plan.xmind -f mm,png -o out    comma-separated also works
  xmind2open plan.xmind -f all -o out/plan  every format into out/plan/")]
struct Cli {
    /// Path to the .xmind workbook
    #[arg(value_name = "FILE")]
    input: PathBuf,

    /// Output format; repeat or comma-separate for several
    #[arg(
        short,
        long = "format",
        value_enum,
        value_name = "FORMAT",
        value_delimiter = ',',
        default_value = "opml",
        hide_default_value = true
    )]
    formats: Vec<Format>,

    /// Directory to write into (created if missing)
    #[arg(short, long, value_name = "DIR", default_value = ".")]
    output: PathBuf,

    /// Only print errors
    #[arg(short, long)]
    quiet: bool,
}

fn slug(title: &str) -> String {
    let re = Regex::new(r"[^\w\- ]+").unwrap();
    re.replace_all(title, "").trim().replace(' ', "-")
}

/// File stem for sheet `i`; plain document title when there is only one sheet.
fn sheet_base(doc_title: &str, sheets: &[Sheet], i: usize) -> String {
    if sheets.len() > 1 {
        format!("{doc_title} - {:02}-{}", i + 1, slug(&sheets[i].title))
    } else {
        doc_title.to_string()
    }
}

fn write(written: &mut Vec<PathBuf>, dir: &Path, name: String, text: String) -> Result<()> {
    let path = dir.join(name);
    fs::write(&path, text).with_context(|| format!("cannot write {}", path.display()))?;
    written.push(path);
    Ok(())
}

fn run(cli: Cli) -> Result<()> {
    let doc_title = cli
        .input
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "mindmap".into());
    let sheets = load::load(&cli.input)?;
    if sheets.is_empty() {
        bail!("{}: workbook has no sheets", cli.input.display());
    }

    let mut formats: Vec<Format> = Vec::new();
    for f in &cli.formats {
        let expanded: &[Format] = if *f == Format::All { &ALL } else { std::slice::from_ref(f) };
        for f in expanded {
            if !formats.contains(f) {
                formats.push(*f);
            }
        }
    }

    fs::create_dir_all(&cli.output)
        .with_context(|| format!("cannot create {}", cli.output.display()))?;
    let dir = cli.output.as_path();
    let mut written = Vec::new();

    for f in &formats {
        match f {
            Format::Opml => {
                let created = chrono::Utc::now().format("%a, %d %b %Y %H:%M:%S +0000").to_string();
                write(&mut written, dir, format!("{doc_title}.opml"), emit::opml(&sheets, &doc_title, &created))?;
            }
            Format::Md => write(&mut written, dir, format!("{doc_title}.md"), emit::markdown(&sheets, &doc_title))?,
            Format::Dot => write(&mut written, dir, format!("{doc_title}.dot"), emit::dot(&sheets, "TB"))?,
            Format::Html => write(&mut written, dir, format!("{doc_title}.html"), html::html(&sheets, &doc_title))?,
            Format::Mm => {
                for (i, s) in sheets.iter().enumerate() {
                    let name = format!("{}.mm", sheet_base(&doc_title, &sheets, i));
                    write(&mut written, dir, name, emit::freemind(s))?;
                }
            }
            Format::Mermaid => {
                for (i, s) in sheets.iter().enumerate() {
                    let name = format!("{}.mmd", sheet_base(&doc_title, &sheets, i));
                    write(&mut written, dir, name, emit::mermaid(s))?;
                }
            }
            Format::Png | Format::Jpg => {
                let want_png = formats.contains(&Format::Png);
                // JPG is converted from a PNG; if PNG wasn't requested, use a temp file
                for (i, s) in sheets.iter().enumerate() {
                    let base = sheet_base(&doc_title, &sheets, i);
                    let png = if want_png {
                        dir.join(format!("{base}.png"))
                    } else {
                        std::env::temp_dir().join(format!("xmind2open-{}-{i}.png", std::process::id()))
                    };
                    if *f == Format::Png {
                        render::png(s, &png)?;
                        written.push(png);
                    } else {
                        if !want_png {
                            render::png(s, &png)?;
                        }
                        let jpg = dir.join(format!("{base}.jpg"));
                        let res = render::jpg(&png, &jpg);
                        if !want_png {
                            let _ = fs::remove_file(&png);
                        }
                        res?;
                        written.push(jpg);
                    }
                }
            }
            Format::All => unreachable!("expanded above"),
        }
    }

    if !cli.quiet {
        let mut total = Counts::default();
        for s in &sheets {
            total += s.root.count();
        }
        for p in &written {
            println!("{}", p.display());
        }
        eprintln!(
            "{} sheet(s), {} topics, {} notes, {} summaries, {} boundaries",
            sheets.len(), total.topics, total.notes, total.summaries, total.boundaries
        );
    }
    Ok(())
}

fn main() {
    if let Err(e) = run(Cli::parse()) {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}
