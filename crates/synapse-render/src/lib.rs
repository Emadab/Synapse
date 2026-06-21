//! # synapse-render
//!
//! Turns a note + a card template into the HTML shown during study, following
//! Anki's template language: `{{Field}}`, `{{FrontSide}}`,
//! `{{#Field}}…{{/Field}}` / `{{^Field}}…{{/Field}}` conditionals,
//! `{{cloze:Field}}`, `{{type:Field}}`, and the `text:` filter. LaTeX
//! delimiters are left intact for the webview to typeset (KaTeX). Pure and
//! UI-free, so it is reused by desktop, mobile and export preview.

use std::sync::OnceLock;

use regex::Regex;

/// A card template (front/back format strings).
pub struct Template<'a> {
    pub qfmt: &'a str,
    pub afmt: &'a str,
}

/// Everything needed to render one card.
pub struct RenderRequest<'a> {
    pub template: Template<'a>,
    /// Field (name, value) pairs, in note order.
    pub fields: &'a [(String, String)],
    /// Template index — for cloze, the active cloze number is `card_ord + 1`.
    pub card_ord: u16,
    pub is_cloze: bool,
}

/// Rendered HTML for both sides of a card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    pub question: String,
    pub answer: String,
}

/// Render a card's question and answer HTML.
pub fn render(req: &RenderRequest<'_>) -> Rendered {
    let active = req.card_ord + 1;
    let question = normalize_latex(&render_side(req.template.qfmt, req, None, active, false));
    let answer = normalize_latex(&render_side(
        req.template.afmt,
        req,
        Some(&question),
        active,
        true,
    ));
    Rendered { question, answer }
}

/// Convert Anki legacy LaTeX delimiters to KaTeX-compatible ones.
/// * `[latex]…[/latex]` → `\[…\]`
/// * `[$]…[$]`           → `\(…\)`
/// * `[$$]…[$$]`         → `\[…\]`
///
/// Modern Anki already uses `\(…\)` and `\[…\]`, so most notes are unaffected.
pub fn normalize_latex(html: &str) -> String {
    static RE_DISPLAY: OnceLock<Regex> = OnceLock::new();
    static RE_INLINE: OnceLock<Regex> = OnceLock::new();
    static RE_DISPLAY2: OnceLock<Regex> = OnceLock::new();

    let re_display =
        RE_DISPLAY.get_or_init(|| Regex::new(r"(?s)\[latex\](.*?)\[/latex\]").unwrap());
    let re_inline = RE_INLINE.get_or_init(|| Regex::new(r"(?s)\[\$\](.*?)\[\$\]").unwrap());
    let re_display2 = RE_DISPLAY2.get_or_init(|| Regex::new(r"(?s)\[\$\$\](.*?)\[\$\$\]").unwrap());

    let s = re_display.replace_all(html, |c: &regex::Captures<'_>| format!("\\[{}\\]", &c[1]));
    let s = re_display2.replace_all(&s, |c: &regex::Captures<'_>| format!("\\[{}\\]", &c[1]));
    re_inline
        .replace_all(&s, |c: &regex::Captures<'_>| format!("\\({}\\)", &c[1]))
        .into_owned()
}

/// Extract sound filenames from `[sound:name]` markers, in document order.
pub fn extract_sounds(html: &str) -> Vec<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"\[sound:([^\]]+)\]").unwrap());
    re.captures_iter(html).map(|c| c[1].to_string()).collect()
}

fn field_value<'a>(fields: &'a [(String, String)], name: &str) -> Option<&'a str> {
    fields
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, v)| v.as_str())
}

fn render_side(
    template: &str,
    req: &RenderRequest<'_>,
    front_side: Option<&str>,
    active_cloze: u16,
    reveal: bool,
) -> String {
    let with_conditionals = apply_conditionals(template, req.fields);
    replace_tokens(&with_conditionals, |inner| {
        let inner = inner.trim();
        if inner == "FrontSide" {
            return front_side.unwrap_or("").to_string();
        }
        // `filter:filter:Field` — the field name is the last segment.
        let mut parts: Vec<&str> = inner.split(':').collect();
        let name = parts.pop().unwrap_or("").trim();
        let filters = parts;
        let value = field_value(req.fields, name).unwrap_or("");

        if filters.iter().any(|f| f.trim() == "cloze")
            || (req.is_cloze && filters.is_empty() && is_cloze_field(value))
        {
            return process_cloze(value, active_cloze, reveal);
        }
        if filters.iter().any(|f| f.trim() == "type") {
            return if reveal {
                format!("<div class=\"synapse-typed-answer\">{value}</div>")
            } else {
                "<input class=\"synapse-type-input\" autocomplete=\"off\" autocapitalize=\"off\" />"
                    .to_string()
            };
        }
        if filters.iter().any(|f| f.trim() == "text") {
            return strip_html(value);
        }
        value.to_string()
    })
}

/// Expand `{{#F}}…{{/F}}` (shown when F is non-empty) and `{{^F}}…{{/F}}`
/// (shown when F is empty). Applied repeatedly so adjacent/sequential sections
/// all resolve. Truly nested same-tag sections are uncommon and not handled.
fn apply_conditionals(template: &str, fields: &[(String, String)]) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"(?s)\{\{([#^])([^{}]+?)\}\}(.*?)\{\{/\s*([^{}]+?)\s*\}\}").unwrap()
    });

    let mut current = template.to_string();
    loop {
        let mut changed = false;
        let next = re
            .replace_all(&current, |caps: &regex::Captures<'_>| {
                changed = true;
                let kind = &caps[1];
                let name = caps[2].trim();
                let body = &caps[3];
                let non_empty = field_value(fields, name)
                    .map(|v| !v.trim().is_empty())
                    .unwrap_or(false);
                let keep = if kind == "#" { non_empty } else { !non_empty };
                if keep {
                    body.to_string()
                } else {
                    String::new()
                }
            })
            .into_owned();
        current = next;
        if !changed {
            return current;
        }
    }
}

fn replace_tokens(template: &str, mut resolve: impl FnMut(&str) -> String) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"\{\{(.+?)\}\}").unwrap());
    re.replace_all(template, |caps: &regex::Captures<'_>| resolve(&caps[1]))
        .into_owned()
}

fn is_cloze_field(value: &str) -> bool {
    value.contains("{{c") && value.contains("::")
}

/// Replace `{{cN::text::hint}}` markers. On the question, the active cloze is
/// hidden (`[hint]` or `[...]`) and the rest show their text; on the answer all
/// are revealed, with the active one highlighted.
fn process_cloze(value: &str, active: u16, reveal: bool) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"(?s)\{\{c(\d+)::(.*?)(?:::(.*?))?\}\}").unwrap());
    re.replace_all(value, |caps: &regex::Captures<'_>| {
        let n: u16 = caps[1].parse().unwrap_or(0);
        let text = &caps[2];
        let hint = caps.get(3).map(|m| m.as_str()).filter(|h| !h.is_empty());
        if n == active {
            if reveal {
                format!("<span class=\"cloze\">{text}</span>")
            } else {
                match hint {
                    Some(h) => format!("<span class=\"cloze\">[{h}]</span>"),
                    None => "<span class=\"cloze\">[...]</span>".to_string(),
                }
            }
        } else {
            text.to_string()
        }
    })
    .into_owned()
}

fn strip_html(value: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"<[^>]*>").unwrap());
    re.replace_all(value, "").into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(n, v)| (n.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn basic_front_and_back_with_frontside() {
        let f = fields(&[("Front", "hola"), ("Back", "hello")]);
        let r = render(&RenderRequest {
            template: Template {
                qfmt: "{{Front}}",
                afmt: "{{FrontSide}}<hr>{{Back}}",
            },
            fields: &f,
            card_ord: 0,
            is_cloze: false,
        });
        assert_eq!(r.question, "hola");
        assert_eq!(r.answer, "hola<hr>hello");
    }

    #[test]
    fn conditional_sections() {
        let shown = render(&RenderRequest {
            template: Template {
                qfmt: "{{Front}}{{#Extra}} ({{Extra}}){{/Extra}}",
                afmt: "x",
            },
            fields: &fields(&[("Front", "q"), ("Extra", "note")]),
            card_ord: 0,
            is_cloze: false,
        });
        assert_eq!(shown.question, "q (note)");

        let hidden = render(&RenderRequest {
            template: Template {
                qfmt: "{{Front}}{{#Extra}} ({{Extra}}){{/Extra}}",
                afmt: "x",
            },
            fields: &fields(&[("Front", "q"), ("Extra", "")]),
            card_ord: 0,
            is_cloze: false,
        });
        assert_eq!(hidden.question, "q");
    }

    #[test]
    fn cloze_hides_active_reveals_on_back() {
        let f = fields(&[("Text", "{{c1::Paris}} is in {{c2::France}}")]);
        let r = render(&RenderRequest {
            template: Template {
                qfmt: "{{cloze:Text}}",
                afmt: "{{cloze:Text}}",
            },
            fields: &f,
            card_ord: 0, // active cloze = c1
            is_cloze: true,
        });
        assert_eq!(
            r.question,
            "<span class=\"cloze\">[...]</span> is in France"
        );
        assert_eq!(r.answer, "<span class=\"cloze\">Paris</span> is in France");
    }

    #[test]
    fn cloze_hint_is_used() {
        let f = fields(&[("Text", "{{c1::Paris::capital}}")]);
        let r = render(&RenderRequest {
            template: Template {
                qfmt: "{{cloze:Text}}",
                afmt: "{{cloze:Text}}",
            },
            fields: &f,
            card_ord: 0,
            is_cloze: true,
        });
        assert_eq!(r.question, "<span class=\"cloze\">[capital]</span>");
    }

    #[test]
    fn type_field_is_input_then_answer() {
        let f = fields(&[("Front", "q"), ("Back", "answer")]);
        let r = render(&RenderRequest {
            template: Template {
                qfmt: "{{type:Back}}",
                afmt: "{{type:Back}}",
            },
            fields: &f,
            card_ord: 0,
            is_cloze: false,
        });
        assert!(r.question.contains("synapse-type-input"));
        assert!(r.answer.contains("answer"));
    }

    #[test]
    fn normalize_latex_converts_legacy_forms() {
        assert_eq!(normalize_latex("[latex]E=mc^2[/latex]"), "\\[E=mc^2\\]");
        assert_eq!(normalize_latex("[$]x^2[$]"), "\\(x^2\\)");
        assert_eq!(normalize_latex("[$$]\\sum n[$$]"), "\\[\\sum n\\]");
        // Modern forms pass through unchanged.
        let modern = "\\(x\\) and \\[y\\]";
        assert_eq!(normalize_latex(modern), modern);
    }

    #[test]
    fn extract_sounds_returns_ordered_filenames() {
        let html = "word [sound:a.mp3] more [sound:b.ogg] end";
        assert_eq!(extract_sounds(html), vec!["a.mp3", "b.ogg"]);
        assert_eq!(extract_sounds("no sounds here"), Vec::<String>::new());
    }

    #[test]
    fn text_filter_strips_html() {
        let f = fields(&[("Front", "<b>bold</b> text")]);
        let r = render(&RenderRequest {
            template: Template {
                qfmt: "{{text:Front}}",
                afmt: "x",
            },
            fields: &f,
            card_ord: 0,
            is_cloze: false,
        });
        assert_eq!(r.question, "bold text");
    }
}
