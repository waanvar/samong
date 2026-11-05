//! The design system, held to what `docs/DESIGN.md` says it is.
//!
//! A screen added a year from now — by someone else, or by an agent told only
//! "add a menu for X" — is built from that document. A design document is also the
//! kind of file that stops being true quietly: a token changes in the stylesheet,
//! the table keeps the old value, and the next screen is built faithfully to a
//! palette that no longer exists. Nothing about a green build says so.
//!
//! So the parts of the document that can be checked mechanically are checked here,
//! against the files the binary actually embeds:
//!
//! - the token table, cell by cell, against `:root` and `:root[data-theme="light"]`
//!   in `web/src/styles.css`;
//! - that no rule outside those two blocks writes a colour literal, which is what
//!   keeps every screen on the same palette and both themes working;
//! - that colour is never transitioned — the bug that left the search field dark in
//!   light mode at 1.11:1 contrast (PLAN.md, Phase 16);
//! - that every animation waits for `prefers-reduced-motion: no-preference`;
//! - that the four vault hues agree across the stylesheet, the graph canvas (which
//!   cannot read CSS variables for them) and the website, three copies that matched
//!   only because nobody had changed one yet.
//!
//! What it cannot check — whether a new screen chose the right *meaning* for its
//! colours, or the right shape from the recipe — stays a matter of reading the
//! document. This test keeps the document worth reading.

use std::collections::BTreeMap;
use std::fs;

fn repo_file(name: &str) -> String {
    let path = format!("{}/{name}", env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {path}: {e}"))
}

const APP_CSS: &str = "web/src/styles.css";
const DARK: &str = ":root";
const LIGHT: &str = ":root[data-theme=\"light\"]";

/// One declaration and the chain of selectors / at-rules it sits inside.
#[derive(Debug)]
struct Decl {
    context: Vec<String>,
    property: String,
    value: String,
}

fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn strip_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        rest = match rest[start + 2..].find("*/") {
            Some(end) => &rest[start + 2 + end + 2..],
            None => "",
        };
    }
    out.push_str(rest);
    out
}

/// Enough of a CSS reader for this stylesheet: nesting by braces, declarations by
/// semicolons. The stylesheet has no strings containing either, which the
/// sanity checks below would notice if it ever did (tokens would go missing).
fn declarations(css: &str) -> Vec<Decl> {
    let css = strip_comments(css);
    let mut context: Vec<String> = Vec::new();
    let mut buf = String::new();
    let mut out = Vec::new();
    let flush = |buf: &mut String, context: &Vec<String>, out: &mut Vec<Decl>| {
        let text = buf.trim();
        if let Some((property, value)) = text.split_once(':') {
            out.push(Decl {
                context: context.clone(),
                property: property.trim().to_string(),
                value: squash(value),
            });
        }
        buf.clear();
    };
    for c in css.chars() {
        match c {
            '{' => {
                context.push(squash(&buf));
                buf.clear();
            }
            ';' => flush(&mut buf, &context, &mut out),
            '}' => {
                flush(&mut buf, &context, &mut out);
                context.pop();
            }
            _ => buf.push(c),
        }
    }
    out
}

fn tokens(decls: &[Decl], selector: &str) -> BTreeMap<String, String> {
    decls
        .iter()
        .filter(|d| d.context == [selector] && d.property.starts_with("--"))
        .map(|d| (d.property.clone(), d.value.clone()))
        .collect()
}

fn app_decls() -> Vec<Decl> {
    declarations(&repo_file(APP_CSS))
}

/// The first `<style>` block of the website.
fn site_decls() -> Vec<Decl> {
    let html = repo_file("site/index.html");
    let start = html
        .find("<style>")
        .expect("site/index.html has a <style> block")
        + 7;
    let end = start + html[start..].find("</style>").expect("closing </style>");
    declarations(&html[start..end])
}

fn contains_hex_colour(value: &str) -> bool {
    value.match_indices('#').any(|(i, _)| {
        let hex = value[i + 1..]
            .chars()
            .take_while(char::is_ascii_hexdigit)
            .count();
        matches!(hex, 3 | 4 | 6 | 8)
    })
}

#[test]
fn the_token_table_in_the_design_doc_matches_the_stylesheet() {
    let decls = app_decls();
    let dark = tokens(&decls, DARK);
    let light = tokens(&decls, LIGHT);
    // A reader that silently found nothing would make every comparison below pass.
    assert!(dark.len() >= 30, "found only {} dark tokens", dark.len());
    assert!(light.len() >= 15, "found only {} light tokens", light.len());

    let doc = repo_file("docs/DESIGN.md");
    let mut rows: BTreeMap<String, (String, String)> = BTreeMap::new();
    for line in doc.lines().filter(|l| l.starts_with("| `--")) {
        let cells: Vec<String> = line
            .split('|')
            .map(|c| c.trim().trim_matches('`').to_string())
            .collect();
        // ["", token, dark, light, role, ""]
        assert!(cells.len() >= 5, "malformed token row: {line}");
        let previous = rows.insert(cells[1].clone(), (squash(&cells[2]), squash(&cells[3])));
        assert!(previous.is_none(), "{} is listed twice", cells[1]);
    }

    let mut problems = Vec::new();
    for (name, value) in &dark {
        match rows.get(name) {
            None => problems.push(format!("{name} is in {APP_CSS} but not in the table")),
            Some((doc_dark, doc_light)) => {
                if doc_dark != value {
                    problems.push(format!(
                        "{name}: dark is `{value}`, the table says `{doc_dark}`"
                    ));
                }
                let expected_light = light.get(name).map_or("—", String::as_str);
                if doc_light != expected_light {
                    problems.push(format!(
                        "{name}: light is `{expected_light}`, the table says `{doc_light}`"
                    ));
                }
            }
        }
    }
    for name in rows.keys().filter(|n| !dark.contains_key(*n)) {
        problems.push(format!("{name} is in the table but no longer in {APP_CSS}"));
    }
    for name in light.keys().filter(|n| !dark.contains_key(*n)) {
        problems.push(format!("{name} exists only in the light theme"));
    }
    assert!(
        problems.is_empty(),
        "docs/DESIGN.md and {APP_CSS} disagree:\n  {}",
        problems.join("\n  ")
    );
}

#[test]
fn colours_are_written_only_in_the_token_blocks() {
    let offenders: Vec<String> = app_decls()
        .into_iter()
        .filter(|d| d.context != [DARK] && d.context != [LIGHT])
        .filter(|d| {
            let v = d.value.to_ascii_lowercase();
            contains_hex_colour(&v)
                || v.contains("rgb(")
                || v.contains("rgba(")
                || v.contains("hsl(")
                || v.contains("hsla(")
        })
        .map(|d| format!("{} {{ {}: {} }}", d.context.join(" "), d.property, d.value))
        .collect();
    assert!(
        offenders.is_empty(),
        "colour literals outside the token blocks — add or reuse a token instead:\n  {}",
        offenders.join("\n  ")
    );
}

#[test]
fn colour_is_never_transitioned() {
    let colourish = |p: &str| {
        p == "all"
            || p.contains("color")
            || p.starts_with("background")
            || p.starts_with("border")
            || p.starts_with("outline")
            || p == "box-shadow"
            || p == "fill"
            || p == "stroke"
            // `transition: 200ms` names no property, which means all of them.
            || p.starts_with(|c: char| c.is_ascii_digit() || c == '.')
    };
    let mut offenders = Vec::new();
    for d in app_decls() {
        if d.property != "transition" && d.property != "transition-property" {
            continue;
        }
        for part in d.value.split(',') {
            let property = part.split_whitespace().next().unwrap_or("");
            if colourish(property) {
                offenders.push(format!(
                    "{}: {}: {}",
                    d.context.join(" "),
                    d.property,
                    d.value
                ));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "colour is transitioned, which strands it on the old theme when the theme flips:\n  {}",
        offenders.join("\n  ")
    );
}

#[test]
fn every_animation_waits_for_reduced_motion_to_be_off() {
    let decls = app_decls();
    let animations: Vec<&Decl> = decls
        .iter()
        .filter(|d| d.property == "animation" || d.property == "animation-name")
        .collect();
    assert!(!animations.is_empty(), "found no animations at all");
    let unguarded: Vec<String> = animations
        .iter()
        .filter(|d| {
            !d.context
                .iter()
                .any(|c| c.contains("prefers-reduced-motion: no-preference"))
        })
        .map(|d| format!("{}: {}", d.context.join(" "), d.value))
        .collect();
    assert!(
        unguarded.is_empty(),
        "animations outside @media (prefers-reduced-motion: no-preference):\n  {}",
        unguarded.join("\n  ")
    );
}

#[test]
fn vault_hues_agree_across_the_stylesheet_the_canvas_and_the_site() {
    let hues = |map: &BTreeMap<String, String>| -> Vec<String> {
        (1..=4)
            .map(|i| {
                map.get(&format!("--v{i}"))
                    .unwrap_or_else(|| panic!("--v{i} is not defined"))
                    .to_ascii_lowercase()
            })
            .collect()
    };
    let app = hues(&tokens(&app_decls(), DARK));

    let canvas_src = repo_file("web/src/components/GraphCanvas.tsx");
    let line = canvas_src
        .lines()
        .find(|l| l.trim_start().starts_with("const VAULT_HUES"))
        .expect("GraphCanvas.tsx declares VAULT_HUES");
    let canvas: Vec<String> = line
        .split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_ascii_lowercase)
        .collect();

    let site = hues(&tokens(&site_decls(), DARK));

    assert_eq!(
        canvas, app,
        "VAULT_HUES in GraphCanvas.tsx differs from --v1..--v4"
    );
    assert_eq!(
        site, app,
        "site/index.html's vault hues differ from the app's"
    );
}

#[test]
fn the_site_keeps_the_apps_dark_palette() {
    let app = tokens(&app_decls(), DARK);
    let site = tokens(&site_decls(), DARK);
    let shared: Vec<&String> = site.keys().filter(|k| app.contains_key(*k)).collect();
    assert!(
        shared.len() >= 10,
        "only {} shared tokens found",
        shared.len()
    );
    let differing: Vec<String> = shared
        .into_iter()
        .filter(|k| site[*k] != app[*k])
        .map(|k| format!("{k}: site `{}`, app `{}`", site[k], app[k]))
        .collect();
    assert!(
        differing.is_empty(),
        "a token shared by the site and the app has drifted:\n  {}",
        differing.join("\n  ")
    );
}

#[test]
fn the_reader_sees_through_comments_and_nesting() {
    let css = "/* a { color: #fff; } */ :root { --a: #000; }\n\
               @media (x) { .b { animation: y 1s; } }";
    let d = declarations(css);
    assert_eq!(d.len(), 2);
    assert_eq!(d[0].context, [":root"]);
    assert_eq!(d[1].context, ["@media (x)", ".b"]);
    assert!(contains_hex_colour("1px solid #39414e"));
    assert!(!contains_hex_colour("url(#gradient)"));
}
