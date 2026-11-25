//! Import und Export im Netscape-Bookmark-Format (wie es alle Browser kennen).
//!
//! Aufbau: `<DL>` enthält `<DT>`-Einträge; ein Ordner ist `<DT><H3>Name</H3><DL>…</DL>`,
//! ein Bookmark `<DT><A HREF="…" ADD_DATE="…">Titel</A>`. Der HTML-Parser ordnet
//! die `<DL>` je nach Datei innerhalb oder neben dem `<DT>` ein; beides wird verstanden.

use std::fmt::Write;

use scraper::{ElementRef, Html};

use crate::library::{Bookmark, Folder};

/// Höchstzahl an Bookmarks bzw. Ordnern pro Import.
pub const MAX_BOOKMARKS: usize = 50_000;
pub const MAX_FOLDERS: usize = 10_000;
/// Tiefer verschachtelte Ordner werden in den Ordner auf dieser Ebene gelegt.
pub const MAX_DEPTH: usize = 32;

#[derive(Debug, Default, PartialEq)]
pub struct ImportFolder {
    pub name: String,
    pub add_date: Option<i64>,
    pub folders: Vec<ImportFolder>,
    pub bookmarks: Vec<ImportBookmark>,
}

#[derive(Debug, PartialEq)]
pub struct ImportBookmark {
    pub url: String,
    pub title: String,
    pub add_date: Option<i64>,
}

#[derive(Default)]
struct Counter {
    bookmarks: usize,
    folders: usize,
    /// Einträge, die wegen der Grenzen weggelassen wurden.
    dropped: usize,
}

/// Höchste Rekursionstiefe beim Durchlaufen des Dokuments (zusätzlich zur
/// Prüfung der Verschachtelung vor dem Parsen).
const MAX_RECURSION: usize = 2 * crate::meta::MAX_NESTING;

/// Liest die Datei. Der Wurzelordner (ohne Namen) steht für die oberste Ebene.
/// Zu tief verschachtelte Dokumente werden abgelehnt (Parser und Rekursion
/// wären sonst angreifbar).
pub fn parse(html: &str) -> Result<(ImportFolder, usize), &'static str> {
    if !crate::meta::nesting_within(html, crate::meta::MAX_NESTING) {
        return Err("Die Datei ist zu tief verschachtelt.");
    }
    let document = Html::parse_document(html);
    let mut root = ImportFolder::default();
    let mut counter = Counter::default();
    walk(document.root_element(), &mut root, 0, 0, &mut counter);
    Ok((root, counter.dropped))
}

/// Durchläuft die Kinder von `element`. Gibt einen gerade per `<H3>` angelegten
/// Ordner zurück, dessen `<DL>` noch nicht gefunden wurde (Index in `current.folders`).
fn walk(
    element: ElementRef,
    current: &mut ImportFolder,
    depth: usize,
    level: usize,
    counter: &mut Counter,
) -> Option<usize> {
    if level > MAX_RECURSION {
        counter.dropped += 1;
        return None;
    }
    let mut pending: Option<usize> = None;
    for child in element.children().filter_map(ElementRef::wrap) {
        visit(child, current, depth, level, counter, &mut pending);
    }
    pending
}

/// Verarbeitet ein Element. `pending` ist ein gerade per `<H3>` angelegter
/// Ordner (Index in `current.folders`), dessen `<DL>` noch folgen kann.
fn visit(
    child: ElementRef,
    current: &mut ImportFolder,
    depth: usize,
    level: usize,
    counter: &mut Counter,
    pending: &mut Option<usize>,
) {
    match child.value().name() {
        "h3" => {
            if counter.folders >= MAX_FOLDERS || depth >= MAX_DEPTH {
                // Inhalt landet im aktuellen Ordner.
                if counter.folders >= MAX_FOLDERS {
                    counter.dropped += 1;
                }
                *pending = None;
                return;
            }
            counter.folders += 1;
            current.folders.push(ImportFolder {
                name: heading_text(child),
                add_date: date(child),
                ..Default::default()
            });
            let index = current.folders.len() - 1;
            *pending = Some(index);

            // Kaputte Dateien: Ein nicht geschlossenes <H3> enthält seine <DL>
            // und alle folgenden Geschwister. Bis einschließlich der ersten <DL>
            // gehört alles in diesen Ordner, der Rest in die aktuelle Ebene.
            let mut list_seen = false;
            for inner in child.children().filter_map(ElementRef::wrap) {
                if list_seen {
                    visit(inner, current, depth, level + 1, counter, pending);
                } else {
                    let is_list = inner.value().name() == "dl";
                    let folder = &mut current.folders[index];
                    let mut inner_pending = None;
                    if is_list {
                        walk(inner, folder, depth + 1, level + 1, counter);
                        list_seen = true;
                        *pending = None;
                    } else if inner.value().name() == "a" {
                        // Ein <A> in einer Überschrift ist eine Kopie eines nicht
                        // geschlossenen Links davor, kein eigenes Bookmark.
                        for nested in inner.children().filter_map(ElementRef::wrap) {
                            visit(
                                nested,
                                folder,
                                depth + 1,
                                level + 1,
                                counter,
                                &mut inner_pending,
                            );
                        }
                    } else {
                        visit(
                            inner,
                            folder,
                            depth + 1,
                            level + 1,
                            counter,
                            &mut inner_pending,
                        );
                    }
                }
            }
        }
        "dl" => match pending.take() {
            Some(index) => {
                let folder = &mut current.folders[index];
                walk(child, folder, depth + 1, level + 1, counter);
            }
            None => {
                walk(child, current, depth, level + 1, counter);
            }
        },
        "a" => {
            // Kopie eines nicht geschlossenen <A>, die der Parser um folgende
            // Einträge legt (kein eigener Text, nur Struktur): nur der Inhalt zählt.
            let own_text = child
                .children()
                .any(|node| node.value().as_text().is_some_and(|t| !t.trim().is_empty()));
            let has_elements = child.children().any(|node| node.value().is_element());
            if !own_text && has_elements {
                for inner in child.children().filter_map(ElementRef::wrap) {
                    visit(inner, current, depth, level + 1, counter, pending);
                }
                return;
            }
            let Some(url) = child.value().attr("href") else {
                return;
            };
            if counter.bookmarks >= MAX_BOOKMARKS {
                counter.dropped += 1;
                return;
            }
            // Kopien desselben Links direkt hintereinander (entstehen beim
            // Parsen nicht geschlossener <A>) nur einmal übernehmen.
            let url = url.trim();
            if current.bookmarks.last().is_some_and(|last| last.url == url) {
                return;
            }
            counter.bookmarks += 1;
            current.bookmarks.push(ImportBookmark {
                url: url.to_string(),
                title: text(child),
                add_date: date(child),
            });
            // Kaputte Dateien: Ein nicht geschlossenes <A> enthält die folgenden
            // Einträge. Deren Links nicht doppelt zählen, aber übernehmen.
            if child.children().any(|node| node.value().is_element()) {
                for inner in child.children().filter_map(ElementRef::wrap) {
                    visit(inner, current, depth, level + 1, counter, pending);
                }
            }
        }
        // DT, P, DD, BODY usw.: Inhalt gehört zur selben Ebene. Ein darin
        // angelegter Ordner kann seine DL als nächstes Geschwister haben.
        _ => {
            if let Some(index) = walk(child, current, depth, level + 1, counter) {
                *pending = Some(index);
            }
        }
    }
}

/// Name eines Ordners: nur der eigene Text des <H3>, nicht der von
/// versehentlich darin liegenden Elementen.
fn heading_text(element: ElementRef) -> String {
    let own: String = element
        .children()
        .filter_map(|node| node.value().as_text().map(|text| text.to_string()))
        .collect();
    let own = own.split_whitespace().collect::<Vec<_>>().join(" ");
    if own.is_empty() { text(element) } else { own }
}

/// Liest die Datei als UTF-8. Ältere Exporte (Internet Explorer, Netscape)
/// sind oft Windows-1252 bzw. ISO-8859-1 kodiert: Ist die Datei kein
/// gültiges UTF-8, wird sie so gelesen.
pub fn decode(bytes: &[u8]) -> String {
    if let Ok(text) = std::str::from_utf8(bytes) {
        return text.strip_prefix('\u{feff}').unwrap_or(text).to_string();
    }
    // Windows-1252: 0x80–0x9F weichen von ISO-8859-1 ab.
    const HIGH: [char; 32] = [
        '\u{20ac}', '\u{81}', '\u{201a}', '\u{192}', '\u{201e}', '\u{2026}', '\u{2020}',
        '\u{2021}', '\u{2c6}', '\u{2030}', '\u{160}', '\u{2039}', '\u{152}', '\u{8d}', '\u{17d}',
        '\u{8f}', '\u{90}', '\u{2018}', '\u{2019}', '\u{201c}', '\u{201d}', '\u{2022}', '\u{2013}',
        '\u{2014}', '\u{2dc}', '\u{2122}', '\u{161}', '\u{203a}', '\u{153}', '\u{9d}', '\u{17e}',
        '\u{178}',
    ];
    bytes
        .iter()
        .map(|&byte| match byte {
            0x80..=0x9f => HIGH[usize::from(byte - 0x80)],
            _ => char::from(byte),
        })
        .collect()
}

fn text(element: ElementRef) -> String {
    element
        .text()
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// `ADD_DATE` in Sekunden. Manche Programme schreiben Millisekunden oder Mikrosekunden.
fn date(element: ElementRef) -> Option<i64> {
    let mut value: i64 = element.value().attr("add_date")?.trim().parse().ok()?;
    while value > 100_000_000_000 {
        value /= 1000;
    }
    (value > 0).then_some(value)
}

/// Schreibt alle Ordner und Bookmarks. Alle Texte werden HTML-maskiert.
pub fn export(folders: &[Folder], bookmarks: &[Bookmark]) -> String {
    let mut out = String::from(
        "<!DOCTYPE NETSCAPE-Bookmark-file-1>\n\
         <!-- Exportiert aus SEBO. -->\n\
         <META HTTP-EQUIV=\"Content-Type\" CONTENT=\"text/html; charset=UTF-8\">\n\
         <TITLE>Bookmarks</TITLE>\n\
         <H1>Bookmarks</H1>\n",
    );
    write_level(&mut out, None, folders, bookmarks, 0);
    out
}

fn write_level(
    out: &mut String,
    parent: Option<i64>,
    folders: &[Folder],
    bookmarks: &[Bookmark],
    depth: usize,
) {
    let indent = "    ".repeat(depth);
    out.push_str(&indent);
    out.push_str("<DL><p>\n");
    // Schutz gegen Zyklen in beschädigten Daten.
    if depth < 256 {
        for folder in folders.iter().filter(|f| f.parent_id == parent) {
            let _ = writeln!(
                out,
                "{indent}    <DT><H3 ADD_DATE=\"{}\">{}</H3>",
                folder.created_at,
                escape(&folder.name)
            );
            write_level(out, Some(folder.id), folders, bookmarks, depth + 1);
        }
    }
    for bookmark in bookmarks.iter().filter(|b| b.folder_id == parent) {
        let _ = writeln!(
            out,
            "{indent}    <DT><A HREF=\"{}\" ADD_DATE=\"{}\" LAST_MODIFIED=\"{}\">{}</A>",
            escape(&bookmark.url),
            bookmark.created_at,
            bookmark.updated_at,
            escape(&bookmark.title)
        );
    }
    out.push_str(&indent);
    out.push_str("</DL><p>\n");
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::MetaStatus;

    const FIREFOX: &str = r#"<!DOCTYPE NETSCAPE-Bookmark-file-1>
<META HTTP-EQUIV="Content-Type" CONTENT="text/html; charset=UTF-8">
<TITLE>Bookmarks</TITLE>
<H1>Bookmarks Menu</H1>
<DL><p>
    <DT><A HREF="https://oben.example/" ADD_DATE="1700000000">Oben</A>
    <DT><H3 ADD_DATE="1600000000">Entwicklung</H3>
    <DL><p>
        <DT><A HREF="https://www.rust-lang.org/" ADD_DATE="1700000001123">Rust &amp; Co</A>
        <DT><H3>Leer</H3>
        <DL><p>
        </DL><p>
        <DT><H3>Tief</H3>
        <DL><p>
            <DT><A HREF="javascript:alert(1)">Böse</A>
            <DT><A HREF="place:sort=8">Firefox-Abfrage</A>
        </DL><p>
    </DL><p>
    <DT><A>ohne Adresse</A>
</DL>"#;

    #[test]
    fn liest_ordnerstruktur() {
        let (root, dropped) = parse(FIREFOX).unwrap();
        assert_eq!(dropped, 0);
        assert_eq!(root.bookmarks.len(), 1);
        assert_eq!(root.bookmarks[0].url, "https://oben.example/");
        assert_eq!(root.bookmarks[0].add_date, Some(1_700_000_000));
        assert_eq!(root.folders.len(), 1);
        let dev = &root.folders[0];
        assert_eq!(dev.name, "Entwicklung");
        assert_eq!(dev.add_date, Some(1_600_000_000));
        assert_eq!(dev.bookmarks[0].title, "Rust & Co");
        // Millisekunden werden zu Sekunden
        assert_eq!(dev.bookmarks[0].add_date, Some(1_700_000_001));
        let names: Vec<_> = dev.folders.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["Leer", "Tief"]);
        assert_eq!(dev.folders[1].bookmarks.len(), 2);
    }

    #[test]
    fn dl_neben_dt_wird_erkannt() {
        // Manche Programme schließen das DT, die DL steht dann daneben.
        let html = "<DL><DT><H3>A</H3></DT><DL><DT><A HREF='https://a.example/'>a</A></DT></DL>\
                    <DT><A HREF='https://b.example/'>b</A></DT></DL>";
        let (root, _) = parse(html).unwrap();
        assert_eq!(root.folders.len(), 1);
        assert_eq!(root.folders[0].bookmarks[0].url, "https://a.example/");
        assert_eq!(root.bookmarks[0].url, "https://b.example/");
    }

    #[test]
    fn begrenzt_tiefe() {
        let mut html = String::new();
        for i in 0..(MAX_DEPTH + 10) {
            let _ = write!(html, "<DL><DT><H3>{i}</H3>");
        }
        html.push_str("<DL><DT><A HREF='https://tief.example/'>x</A></DL>");
        let (root, _) = parse(&html).unwrap();
        let mut depth = 0;
        let mut folder = &root;
        while let Some(child) = folder.folders.first() {
            folder = child;
            depth += 1;
        }
        assert_eq!(depth, MAX_DEPTH);
        assert_eq!(folder.bookmarks[0].url, "https://tief.example/");
    }

    #[test]
    fn kaputtes_html() {
        let html = "<DL><p><DT><H3>Offen<DL><DT><A HREF=\"http://10.3.0.1/\">ohne Ende                    <DT><A HREF=http://10.3.0.2/>ohne Anführung</a></DL>                    <DT><H3>Noch</H3><DL><DT><A HREF=\"http://10.3.0.3/\">drin";
        let (root, _) = parse(html).unwrap();
        let mut urls = Vec::new();
        let mut names = Vec::new();
        fn collect<'a>(f: &'a ImportFolder, urls: &mut Vec<&'a str>, names: &mut Vec<&'a str>) {
            urls.extend(f.bookmarks.iter().map(|b| b.url.as_str()));
            for child in &f.folders {
                names.push(child.name.as_str());
                collect(child, urls, names);
            }
        }
        collect(&root, &mut urls, &mut names);
        urls.sort_unstable();
        assert_eq!(
            urls,
            ["http://10.3.0.1/", "http://10.3.0.2/", "http://10.3.0.3/"]
        );
        assert_eq!(names[0], "Offen");
        assert!(names.contains(&"Noch"));
    }

    fn urls(folder: &ImportFolder) -> Vec<String> {
        folder.bookmarks.iter().map(|b| b.url.clone()).collect()
    }

    #[test]
    fn offenes_h3_mit_geschwistern() {
        let html = r#"<DL><DT><H3>F21a<DL><DT><A HREF="http://a/">a</A></DL><DT><H3>F21b</H3><DL><DT><A HREF="http://b/">b</A></DL></DL>"#;
        let (root, _) = parse(html).unwrap();
        let names: Vec<_> = root.folders.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["F21a", "F21b"]);
        assert_eq!(urls(&root.folders[0]), ["http://a/"]);
        assert_eq!(urls(&root.folders[1]), ["http://b/"]);

        let html = r#"<DL><DT><H3>F23a<DL><DT><A HREF="http://a/">a</A></DL><DT><A HREF="http://c/">c</A></DL>"#;
        let (root, _) = parse(html).unwrap();
        assert_eq!(urls(&root.folders[0]), ["http://a/"]);
        assert_eq!(urls(&root), ["http://c/"]);
    }

    #[test]
    fn offene_links_werden_nicht_doppelt_gezaehlt() {
        let html = r#"<DL><DT><H3>F22a</H3><DL><DT><A HREF="http://a/">a<DT><A HREF="http://b/">b</DL><DT><H3>F22b</H3><DL><DT><A HREF="http://c/">c</A></DL></DL>"#;
        let (root, _) = parse(html).unwrap();
        let mut all = Vec::new();
        fn collect(folder: &ImportFolder, all: &mut Vec<String>) {
            all.extend(folder.bookmarks.iter().map(|b| b.url.clone()));
            for child in &folder.folders {
                collect(child, all);
            }
        }
        collect(&root, &mut all);
        all.sort();
        assert_eq!(all, ["http://a/", "http://b/", "http://c/"]);
    }

    #[test]
    fn zeichensatz() {
        assert_eq!(decode("Grüße".as_bytes()), "Grüße");
        assert_eq!(decode(b"Gr\xfc\xdfe aus K\xf6ln \x80"), "Grüße aus Köln €");
        assert_eq!(decode("\u{feff}<DL>".as_bytes()), "<DL>");
    }

    #[test]
    fn tiefe_dateien_werden_abgelehnt() {
        assert!(parse(&"<div>".repeat(10_000)).is_err());
        assert!(parse(&"<dl>".repeat(10_000)).is_err());
        let started = std::time::Instant::now();
        assert!(parse(&"<div>".repeat(4_000_000)).is_err());
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
    }

    #[test]
    fn export_maskiert_und_ist_wieder_lesbar() {
        let folders = vec![Folder {
            id: 1,
            parent_id: None,
            name: "<Ordner> & \"Zitat\"".into(),
            created_at: 5,
        }];
        let bookmark = |id, folder_id, url: &str, title: &str| Bookmark {
            id,
            folder_id,
            url: url.into(),
            title: title.into(),
            image: None,
            icon: None,
            meta_status: MetaStatus::Done,
            created_at: 10,
            updated_at: 11,
        };
        let bookmarks = vec![
            bookmark(
                1,
                Some(1),
                "https://a.example/?x=1&y=\"2\"",
                "<script>alert(1)</script>",
            ),
            bookmark(2, None, "https://b.example/", "B"),
        ];
        let html = export(&folders, &bookmarks);
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
        assert!(html.contains("x=1&amp;y=&quot;2&quot;"));

        let (root, _) = parse(&html).unwrap();
        assert_eq!(root.folders[0].name, "<Ordner> & \"Zitat\"");
        assert_eq!(
            root.folders[0].bookmarks[0].url,
            "https://a.example/?x=1&y=\"2\""
        );
        assert_eq!(
            root.folders[0].bookmarks[0].title,
            "<script>alert(1)</script>"
        );
        assert_eq!(root.bookmarks[0].add_date, Some(10));
    }
}
