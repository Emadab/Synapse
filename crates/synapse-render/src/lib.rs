//! # synapse-render
//!
//! Turns a note + a card template into the HTML shown during study, following
//! Anki's template language: `{{Field}}`, `{{FrontSide}}`,
//! `{{#Field}}…{{/Field}}` / `{{^Field}}…{{/Field}}` conditionals (correctly
//! nested — see `parser`), `{{cloze:Field}}` / `{{cloze-only:Field}}`,
//! `{{type:Field}}` / `{{type:cloze:Field}}`, `{{hint:Field}}`,
//! `{{tts …:Field}}` / `{{tts-voices:}}`, `{{furigana:}}` / `{{kana:}}` /
//! `{{kanji:}}`, the `text:` filter, and the special fields `{{Tags}}`
//! `{{Type}}` `{{Deck}}` `{{Subdeck}}` `{{Card}}` `{{CardFlag}}`. LaTeX
//! delimiters are left intact for the webview to typeset (KaTeX). Pure and
//! UI-free, so it is reused by desktop, mobile and export preview.

mod cloze;
mod filters;
mod parser;

use std::sync::OnceLock;

use regex::Regex;

pub use filters::RenderCtx;

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
    /// Space-joined note tags, for `{{Tags}}` and `{{#Tags}}`.
    pub tags: &'a str,
    pub deck: &'a str,
    pub subdeck: &'a str,
    pub notetype: &'a str,
    /// The active card template's display name, for `{{Card}}`.
    pub card_name: &'a str,
    /// 0 = no flag, 1-7 = flag color, for `{{CardFlag}}`.
    pub flag: u8,
    /// Image-occlusion notetype's `"hideAllGuessOne"` / `"hideOneGuessOne"` mode.
    pub occlusion_mode: &'a str,
}

impl<'a> Default for RenderRequest<'a> {
    fn default() -> Self {
        Self {
            template: Template { qfmt: "", afmt: "" },
            fields: &[],
            card_ord: 0,
            is_cloze: false,
            tags: "",
            deck: "",
            subdeck: "",
            notetype: "",
            card_name: "",
            flag: 0,
            occlusion_mode: "",
        }
    }
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
    let ctx = RenderCtx {
        fields: req.fields,
        tags: req.tags,
        deck: req.deck,
        subdeck: req.subdeck,
        notetype: req.notetype,
        card_name: req.card_name,
        flag: req.flag,
        is_cloze: req.is_cloze,
        active_cloze: active,
        occlusion_mode: req.occlusion_mode,
    };

    let q_nodes = parser::parse(req.template.qfmt);
    let question = normalize_latex(&render_nodes(&q_nodes, &ctx, None, false));

    let a_nodes = parser::parse(req.template.afmt);
    let answer = normalize_latex(&render_nodes(&a_nodes, &ctx, Some(&question), true));

    Rendered { question, answer }
}

fn render_nodes(
    nodes: &[parser::Node],
    ctx: &RenderCtx<'_>,
    front_side: Option<&str>,
    reveal: bool,
) -> String {
    let mut out = String::new();
    for node in nodes {
        render_node(node, ctx, front_side, reveal, &mut out);
    }
    out
}

fn render_node(
    node: &parser::Node,
    ctx: &RenderCtx<'_>,
    front_side: Option<&str>,
    reveal: bool,
    out: &mut String,
) {
    match node {
        parser::Node::Text(t) => out.push_str(t),
        parser::Node::Section { negate, name, body } => {
            let truthy = filters::section_truthy(name, ctx);
            let show = if *negate { !truthy } else { truthy };
            if show {
                for n in body {
                    render_node(n, ctx, front_side, reveal, out);
                }
            }
        }
        parser::Node::Replacement { filters: f, field } => {
            out.push_str(&filters::render_replacement(
                f, field, ctx, front_side, reveal,
            ));
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(n, v)| (n.to_string(), v.to_string()))
            .collect()
    }

    fn req<'a>(qfmt: &'a str, afmt: &'a str, fields: &'a [(String, String)]) -> RenderRequest<'a> {
        RenderRequest {
            template: Template { qfmt, afmt },
            fields,
            ..RenderRequest::default()
        }
    }

    #[test]
    fn basic_front_and_back_with_frontside() {
        let f = fields(&[("Front", "hola"), ("Back", "hello")]);
        let r = render(&req("{{Front}}", "{{FrontSide}}<hr>{{Back}}", &f));
        assert_eq!(r.question, "hola");
        assert_eq!(r.answer, "hola<hr>hello");
    }

    #[test]
    fn conditional_sections() {
        let shown = render(&req(
            "{{Front}}{{#Extra}} ({{Extra}}){{/Extra}}",
            "x",
            &fields(&[("Front", "q"), ("Extra", "note")]),
        ));
        assert_eq!(shown.question, "q (note)");

        let hidden = render(&req(
            "{{Front}}{{#Extra}} ({{Extra}}){{/Extra}}",
            "x",
            &fields(&[("Front", "q"), ("Extra", "")]),
        ));
        assert_eq!(hidden.question, "q");
    }

    #[test]
    fn nested_same_field_conditionals_resolve_independently() {
        let f = fields(&[("Front", "q"), ("Extra", "note")]);
        let r = render(&req(
            "{{#Front}}A{{#Extra}}B{{#Front}}C{{/Front}}D{{/Extra}}E{{/Front}}",
            "x",
            &f,
        ));
        assert_eq!(r.question, "ABCDE");
    }

    #[test]
    fn negated_conditional() {
        let f = fields(&[("Front", "q"), ("Extra", "")]);
        let r = render(&req("{{^Extra}}no extra{{/Extra}}", "x", &f));
        assert_eq!(r.question, "no extra");
    }

    #[test]
    fn unmatched_close_tag_is_literal() {
        let f = fields(&[("Front", "q")]);
        let r = render(&req("{{Front}}{{/Stray}}", "x", &f));
        assert_eq!(r.question, "q{{/Stray}}");
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
            ..RenderRequest::default()
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
            ..RenderRequest::default()
        });
        assert_eq!(r.question, "<span class=\"cloze\">[capital]</span>");
    }

    #[test]
    fn cloze_only_isolates_active_text() {
        let f = fields(&[("Text", "before {{c1::Paris}} after {{c2::France}}")]);
        let r = render(&RenderRequest {
            template: Template {
                qfmt: "{{cloze-only:Text}}",
                afmt: "{{cloze-only:Text}}",
            },
            fields: &f,
            card_ord: 0,
            is_cloze: true,
            ..RenderRequest::default()
        });
        assert_eq!(r.question, "<span class=\"cloze\">[...]</span>");
        assert_eq!(r.answer, "<span class=\"cloze\">Paris</span>");
    }

    #[test]
    fn type_field_is_input_then_answer() {
        let f = fields(&[("Front", "q"), ("Back", "answer")]);
        let r = render(&req("{{type:Back}}", "{{type:Back}}", &f));
        assert!(r.question.contains("synapse-type-input"));
        assert!(r.answer.contains("data-expected=\"answer\""));
    }

    #[test]
    fn type_cloze_combo_uses_active_cloze_answer() {
        let f = fields(&[("Text", "{{c1::Paris}} is in {{c2::France}}")]);
        let r = render(&RenderRequest {
            template: Template {
                qfmt: "{{type:cloze:Text}}",
                afmt: "{{type:cloze:Text}}",
            },
            fields: &f,
            card_ord: 0,
            is_cloze: true,
            ..RenderRequest::default()
        });
        assert!(r.answer.contains("data-expected=\"Paris\""));
    }

    #[test]
    fn hint_filter_renders_reveal_link() {
        let f = fields(&[("Front", "q"), ("Note", "extra info")]);
        let r = render(&req("{{hint:Note}}", "x", &f));
        assert!(r.question.contains("synapse-hint"));
        assert!(r.question.contains("extra info"));
    }

    #[test]
    fn special_fields_resolve() {
        let f = fields(&[("Front", "q")]);
        let r = render(&RenderRequest {
            template: Template {
                qfmt: "{{Deck}}/{{Subdeck}} [{{Tags}}] {{Type}} {{Card}}",
                afmt: "x",
            },
            fields: &f,
            card_ord: 0,
            is_cloze: false,
            tags: "foo bar",
            deck: "Spanish",
            subdeck: "Verbs",
            notetype: "Basic",
            card_name: "Card 1",
            flag: 0,
            occlusion_mode: "",
        });
        assert_eq!(r.question, "Spanish/Verbs [foo bar] Basic Card 1");
    }

    #[test]
    fn card_flag_special_field() {
        let f = fields(&[("Front", "q")]);
        let r = render(&RenderRequest {
            template: Template {
                qfmt: "{{CardFlag}}",
                afmt: "x",
            },
            fields: &f,
            flag: 3,
            ..req("", "", &f)
        });
        assert_eq!(r.question, "flag3");
    }

    #[test]
    fn furigana_filters() {
        let f = fields(&[("Word", "食べる[たべる] 物[もの]")]);
        let ruby = render(&req("{{furigana:Word}}", "x", &f));
        assert_eq!(
            ruby.question,
            "<ruby>食べる<rt>たべる</rt></ruby> <ruby>物<rt>もの</rt></ruby>"
        );

        let kana = render(&req("{{kana:Word}}", "x", &f));
        assert_eq!(kana.question, "たべる もの");

        let kanji = render(&req("{{kanji:Word}}", "x", &f));
        assert_eq!(kanji.question, "食べる 物");
    }

    #[test]
    fn tts_filter_emits_marker() {
        let f = fields(&[("Front", "hola")]);
        let r = render(&req("{{tts en_US voices=Alice:Front}}", "x", &f));
        assert!(r.question.contains("data-lang=\"en_US\""));
        assert!(r.question.contains("data-voices=\"Alice\""));
        assert!(r.question.contains("data-text=\"hola\""));
    }

    #[test]
    fn tts_voices_placeholder() {
        let f = fields(&[("Front", "hola")]);
        let r = render(&req("{{tts-voices:}}", "x", &f));
        assert!(r.question.contains("synapse-tts-voices"));
    }

    #[test]
    fn image_occlusion_shape_passthrough() {
        let f = fields(&[(
            "Occlusion",
            "{{c1::image-occlusion:rect:left=.1:top=.2:width=.3:height=.15}}",
        )]);
        let r = render(&RenderRequest {
            template: Template {
                qfmt: "{{cloze:Occlusion}}",
                afmt: "{{cloze:Occlusion}}",
            },
            fields: &f,
            card_ord: 0,
            is_cloze: true,
            ..RenderRequest::default()
        });
        assert!(r.question.contains("synapse-io-shape"));
        assert!(r.question.contains("data-revealed=\"false\""));
        assert!(r.answer.contains("data-revealed=\"true\""));
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
        let r = render(&req("{{text:Front}}", "x", &f));
        assert_eq!(r.question, "bold text");
    }
}
