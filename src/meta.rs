//! Titel, Vorschaubild und Favicon aus dem HTML einer Seite lesen.

use scraper::{Html, Selector};
use url::Url;

#[derive(Debug, Default, PartialEq)]
pub struct PageMeta {
    pub title: Option<String>,
    pub image: Option<Url>,
    /// Favicon-Kandidaten, beste zuerst. `/favicon.ico` steht immer am Ende.
    pub icons: Vec<Url>,
}

pub const MAX_TITLE_CHARS: usize = 300;
const MAX_ICON_CANDIDATES: usize = 8;

pub fn parse(html: &str, page_url: &Url) -> PageMeta {
    // Titel, Meta-Tags und Icons stehen im Kopf: der Rest der Seite wird nicht gebraucht.
    let head = match find_ascii_ci(html, "<body") {
        Some(end) => &html[..end],
        None => html,
    };
    if !nesting_within(head, MAX_NESTING) {
        tracing::info!("Seite {page_url} ist zu tief verschachtelt, wird nicht ausgewertet");
        return PageMeta {
            icons: page_url.join("/favicon.ico").into_iter().collect(),
            ..Default::default()
        };
    }
    let document = Html::parse_document(head);
    let selector = |s: &str| Selector::parse(s).expect("gültiger Selektor");

    // <base href> berücksichtigen, aber nur http(s).
    let base = document
        .select(&selector("base[href]"))
        .next()
        .and_then(|element| element.value().attr("href"))
        .and_then(|href| page_url.join(href).ok())
        .filter(|url| matches!(url.scheme(), "http" | "https"))
        .unwrap_or_else(|| page_url.clone());
    let resolve = |href: &str| {
        base.join(href.trim())
            .ok()
            .filter(|url| matches!(url.scheme(), "http" | "https"))
    };

    let mut og_title = None;
    let mut image = None;
    let mut twitter_image = None;
    for element in document.select(&selector("meta[content]")) {
        let element = element.value();
        let key = element
            .attr("property")
            .or_else(|| element.attr("name"))
            .unwrap_or_default()
            .to_ascii_lowercase();
        let content = element.attr("content").unwrap_or_default();
        match key.as_str() {
            "og:title" if og_title.is_none() => og_title = clean_title(content),
            "og:image" | "og:image:url" | "og:image:secure_url" if image.is_none() => {
                image = resolve(content)
            }
            "twitter:image" | "twitter:image:src" if twitter_image.is_none() => {
                twitter_image = resolve(content)
            }
            _ => {}
        }
    }

    let title = og_title.or_else(|| {
        document
            .select(&selector("title"))
            .next()
            .and_then(|element| clean_title(&element.text().collect::<String>()))
    });

    let mut candidates: Vec<(u32, u32, Url)> = Vec::new();
    for element in document.select(&selector("link[rel][href]")) {
        let element = element.value();
        let rel = element.attr("rel").unwrap_or_default().to_ascii_lowercase();
        let rels: Vec<&str> = rel.split_ascii_whitespace().collect();
        let Some(url) = element.attr("href").and_then(resolve) else {
            continue;
        };
        let path = url.path().to_ascii_lowercase();
        let kind = element
            .attr("type")
            .unwrap_or_default()
            .to_ascii_lowercase();
        let is_svg = kind.contains("svg") || path.ends_with(".svg");
        let is_png = kind.contains("png") || path.ends_with(".png");

        // Je höher, desto besser. SVG nur als letzte Wahl.
        let score = if rels.contains(&"apple-touch-icon")
            || rels.contains(&"apple-touch-icon-precomposed")
        {
            5
        } else if !rels.contains(&"icon") {
            continue;
        } else if is_svg {
            1
        } else if is_png {
            4
        } else {
            3
        };
        let size = element
            .attr("sizes")
            .and_then(|sizes| sizes.split(['x', 'X']).next())
            .and_then(|width| width.trim().parse().ok())
            .unwrap_or(0);
        candidates.push((score, size, url));
    }
    candidates.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));

    // Nur die besten Kandidaten behalten (eine Seite könnte Tausende enthalten).
    let mut icons: Vec<Url> = Vec::new();
    for (_, _, url) in candidates {
        if icons.len() >= MAX_ICON_CANDIDATES {
            break;
        }
        if !icons.contains(&url) {
            icons.push(url);
        }
    }
    if let Ok(fallback) = page_url.join("/favicon.ico")
        && !icons.contains(&fallback)
    {
        icons.push(fallback);
    }

    PageMeta {
        title,
        image: image.or(twitter_image),
        icons,
    }
}

/// Position von `needle` (ASCII, ohne Groß-/Kleinschreibung) in `text`.
fn find_ascii_ci(text: &str, needle: &str) -> Option<usize> {
    text.as_bytes()
        .windows(needle.len())
        .position(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
}

/// Entfernt Steuerzeichen, fasst Leerraum zusammen und kürzt.
pub fn clean_title(raw: &str) -> Option<String> {
    let cleaned: String = raw
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_TITLE_CHARS)
        .collect();
    (!cleaned.is_empty()).then_some(cleaned)
}

/// Höchste erlaubte Verschachtelungstiefe fremder HTML-Dokumente.
pub const MAX_NESTING: usize = 512;

/// Prüft grob und in linearer Zeit, ob ein Dokument tiefer verschachtelt ist
/// als `limit`. Der HTML-Parser braucht für tief verschachtelte Elemente
/// quadratische Zeit (und der Import rekursiert über den Baum), deshalb werden
/// solche Dokumente vorher abgewiesen. Elemente, die sich selbst schließen
/// oder vom Parser automatisch geschlossen werden (`<p>`, `<dt>`, `<li>` …),
/// zählen nicht.
pub fn nesting_within(html: &str, limit: usize) -> bool {
    const IGNORED: &[&str] = &[
        "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source",
        "track", "wbr", "param", "keygen", "p", "dt", "dd", "li", "option", "optgroup", "tr", "td",
        "th", "thead", "tbody", "tfoot", "colgroup", "rb", "rt", "rp", "html", "head", "body",
    ];
    const RAW_TEXT: &[&str] = &["script", "style", "textarea", "title", "xmp", "noscript"];

    let bytes = html.as_bytes();
    let lower = |range: &[u8]| String::from_utf8_lossy(range).to_ascii_lowercase();
    let find = |from: usize, needle: &[u8]| {
        bytes[from.min(bytes.len())..]
            .windows(needle.len())
            .position(|window| window.eq_ignore_ascii_case(needle))
            .map(|offset| from + offset)
    };
    let mut stack: Vec<String> = Vec::new();
    let mut i = 0;
    while let Some(offset) = bytes[i..].iter().position(|&b| b == b'<') {
        let start = i + offset + 1;
        if bytes[start..].starts_with(b"!--") {
            i = find(start, b"-->").map_or(bytes.len(), |end| end + 3);
            continue;
        }
        let closing = bytes.get(start) == Some(&b'/');
        let name_start = start + usize::from(closing);
        let name_len = bytes[name_start..]
            .iter()
            .take_while(|b| b.is_ascii_alphanumeric())
            .count();
        if name_len == 0 || !bytes[name_start].is_ascii_alphabetic() {
            i = start;
            continue;
        }
        let name = lower(&bytes[name_start..name_start + name_len]);
        let Some(end) = bytes[name_start..].iter().position(|&b| b == b'>') else {
            break;
        };
        let end = name_start + end;
        i = end + 1;

        if closing {
            // Wie der Browser: bis zum passenden offenen Element schließen,
            // sonst ignorieren.
            if let Some(position) = stack.iter().rposition(|open| *open == name) {
                stack.truncate(position);
            }
        } else if RAW_TEXT.contains(&name.as_str()) {
            // Inhalt ist kein HTML (z. B. `a<b` in Skripten).
            let closing_tag = format!("</{name}");
            i = find(i, closing_tag.as_bytes()).unwrap_or(bytes.len());
        } else if bytes[end - 1] != b'/' && !IGNORED.contains(&name.as_str()) {
            stack.push(name);
            if stack.len() > limit {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn liest_titel_bild_und_icons() {
        let html = r#"<!doctype html><html><head>
            <title>  Seiten
              Titel </title>
            <meta property="og:image" content="/bilder/vorschau.jpg">
            <link rel="icon" type="image/svg+xml" href="/icon.svg">
            <link rel="icon" href="/favicon-32.png" sizes="32x32">
            <link rel="icon" href="/favicon-16.png" sizes="16x16">
            <link rel="apple-touch-icon" href="https://cdn.example.com/apple.png">
            <link rel="stylesheet" href="/style.css">
            <link rel="icon" href="javascript:alert(1)">
        </head><body>Inhalt</body></html>"#;
        let meta = parse(html, &Url::parse("https://example.com/artikel/1").unwrap());
        assert_eq!(meta.title.as_deref(), Some("Seiten Titel"));
        assert_eq!(
            meta.image.unwrap().as_str(),
            "https://example.com/bilder/vorschau.jpg"
        );
        let icons: Vec<&str> = meta.icons.iter().map(Url::as_str).collect();
        assert_eq!(
            icons,
            [
                "https://cdn.example.com/apple.png",
                "https://example.com/favicon-32.png",
                "https://example.com/favicon-16.png",
                "https://example.com/icon.svg",
                "https://example.com/favicon.ico",
            ]
        );
    }

    #[test]
    fn og_title_und_twitter_bild() {
        let html = r#"<head><title>Seite</title>
            <meta property="og:title" content="Besserer Titel">
            <meta name="twitter:image" content="https://example.com/t.png"></head>"#;
        let meta = parse(html, &Url::parse("https://example.com/").unwrap());
        assert_eq!(meta.title.as_deref(), Some("Besserer Titel"));
        assert_eq!(meta.image.unwrap().as_str(), "https://example.com/t.png");
        assert_eq!(meta.icons.len(), 1);
    }

    #[test]
    fn verschachtelung() {
        assert!(nesting_within("<div><div><span>x</span></div></div>", 3));
        assert!(!nesting_within(&"<div>".repeat(600), 512));
        // Automatisch geschlossene und leere Elemente zählen nicht
        assert!(nesting_within(
            &"<dt><p><li><br><img src=x>".repeat(5000),
            10
        ));
        // Unpassende schließende Tags verringern die Tiefe nicht
        assert!(!nesting_within(&"<div></span></p>".repeat(600), 512));
        // Skripte und Kommentare werden übersprungen
        assert!(nesting_within(
            &format!(
                "<script>{}</script><!-- {} -->",
                "a<b ".repeat(1000),
                "<div>".repeat(1000)
            ),
            10
        ));
        // Lesezeichen-Export mit vielen Einträgen ist flach
        let export = format!(
            "<DL><p>{}</DL>",
            "<DT><A HREF=\"https://a/\">a</A>
"
            .repeat(10_000)
        );
        assert!(nesting_within(&export, 10));
    }

    #[test]
    fn tiefe_seite_wird_nicht_geparst() {
        let html = format!("<title>T</title>{}", "<div>".repeat(200_000));
        let started = std::time::Instant::now();
        let meta = parse(&html, &Url::parse("https://example.com/").unwrap());
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
        assert_eq!(meta.title, None);
        // Nur der Kopf wird gelesen
        let html = format!("<title>T</title><body>{}", "<div>".repeat(200_000));
        assert_eq!(
            parse(&html, &Url::parse("https://example.com/").unwrap())
                .title
                .as_deref(),
            Some("T")
        );
    }

    #[test]
    fn leere_seite() {
        let meta = parse("", &Url::parse("https://example.com/x").unwrap());
        assert_eq!(meta.title, None);
        assert_eq!(meta.image, None);
        assert_eq!(meta.icons[0].as_str(), "https://example.com/favicon.ico");
    }
}
