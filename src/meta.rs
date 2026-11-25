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

pub fn parse(html: &str, page_url: &Url) -> PageMeta {
    let document = Html::parse_document(html);
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

    let mut icons: Vec<Url> = Vec::new();
    for (_, _, url) in candidates {
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
    fn leere_seite() {
        let meta = parse("", &Url::parse("https://example.com/x").unwrap());
        assert_eq!(meta.title, None);
        assert_eq!(meta.image, None);
        assert_eq!(meta.icons[0].as_str(), "https://example.com/favicon.ico");
    }
}
