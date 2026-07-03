//! Filter chain resolution for `{{filter:filter:Field}}` replacements, plus
//! the render context (special fields: Tags, Deck, Subdeck, Type, Card,
//! CardFlag) that both filters and section-truthiness checks read from.

use std::sync::OnceLock;

use regex::Regex;

use crate::cloze::{process_cloze, process_cloze_only};

/// Everything a template render needs beyond the raw field list.
pub struct RenderCtx<'a> {
    pub fields: &'a [(String, String)],
    pub tags: &'a str,
    pub deck: &'a str,
    pub subdeck: &'a str,
    pub notetype: &'a str,
    pub card_name: &'a str,
    pub flag: u8,
    pub is_cloze: bool,
    pub active_cloze: u16,
    /// `"hideAllGuessOne"` or `"hideOneGuessOne"` — see `cloze::render_occlusion_shape`.
    pub occlusion_mode: &'a str,
}

/// Resolve a field or special-field name to its string value.
pub fn field_value_for<'a>(name: &str, ctx: &RenderCtx<'a>) -> Option<std::borrow::Cow<'a, str>> {
    use std::borrow::Cow;
    match name {
        "Tags" => Some(Cow::Owned(ctx.tags.to_string())),
        "Type" => Some(Cow::Owned(ctx.notetype.to_string())),
        "Deck" => Some(Cow::Owned(ctx.deck.to_string())),
        "Subdeck" => Some(Cow::Owned(ctx.subdeck.to_string())),
        "Card" => Some(Cow::Owned(ctx.card_name.to_string())),
        "CardFlag" => Some(Cow::Owned(if ctx.flag > 0 {
            format!("flag{}", ctx.flag)
        } else {
            String::new()
        })),
        _ => ctx
            .fields
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| Cow::Borrowed(v.as_str())),
    }
}

/// Whether `{{#Name}}`/`{{^Name}}` should be considered "truthy" for `Name`.
pub fn section_truthy(name: &str, ctx: &RenderCtx<'_>) -> bool {
    field_value_for(name, ctx)
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
}

/// Render one `{{filters...:field}}` replacement, given the already-resolved
/// filter chain (outermost-first, as written) and field name.
pub fn render_replacement(
    filters: &[String],
    field: &str,
    ctx: &RenderCtx<'_>,
    front_side: Option<&str>,
    reveal: bool,
) -> String {
    if field == "FrontSide" {
        return front_side.unwrap_or("").to_string();
    }

    let has = |f: &str| filters.iter().any(|x| x == f);
    let value = field_value_for(field, ctx).unwrap_or_default();

    // Structural filters short-circuit the rest of the chain, mirroring Anki
    // where cloze/type/hint/tts dominate the replacement's final shape.
    if has("type") && has("cloze") {
        return render_type(process_cloze_plain_answer(&value, ctx.active_cloze), reveal);
    }
    if has("cloze-only") {
        return process_cloze_only(&value, ctx.active_cloze, reveal);
    }
    if has("cloze") || (ctx.is_cloze && filters.is_empty() && is_cloze_field(&value)) {
        return process_cloze(&value, ctx.active_cloze, reveal, ctx.occlusion_mode);
    }
    if has("type") {
        return render_type(value.to_string(), reveal);
    }
    if has("hint") {
        return render_hint(field, &value);
    }
    if filters.iter().any(|f| f.as_str() == "tts-voices") {
        return "<div class=\"synapse-tts-voices\"></div>".to_string();
    }
    if let Some(tts) = filters
        .iter()
        .find(|f| f.starts_with("tts ") || f.as_str() == "tts")
    {
        return render_tts(tts, &value);
    }

    // Simple, chainable text transforms.
    let mut out = value.to_string();
    for f in filters {
        out = match f.as_str() {
            "text" => strip_html(&out),
            "furigana" => furigana_transform(&out, FuriganaMode::Ruby),
            "kana" => furigana_transform(&out, FuriganaMode::Reading),
            "kanji" => furigana_transform(&out, FuriganaMode::Base),
            _ => out,
        };
    }
    out
}

fn is_cloze_field(value: &str) -> bool {
    value.contains("{{c") && value.contains("::")
}

fn render_type(expected_html: String, reveal: bool) -> String {
    if reveal {
        let expected = strip_html(&expected_html);
        format!(
            "<span class=\"synapse-typeans\" data-expected=\"{}\"></span>",
            html_escape_attr(&expected)
        )
    } else {
        "<input class=\"synapse-type-input\" autocomplete=\"off\" autocapitalize=\"off\" />"
            .to_string()
    }
}

/// For `{{type:cloze:Field}}`: the expected answer is the active cloze's
/// revealed text (joined if multiple clozes share the active index).
fn process_cloze_plain_answer(value: &str, active: u16) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"(?s)\{\{c(\d+)::(.*?)(?:::(.*?))?\}\}").unwrap());
    let mut parts = Vec::new();
    for caps in re.captures_iter(value) {
        let n: u16 = caps[1].parse().unwrap_or(0);
        if n == active {
            parts.push(caps[2].to_string());
        }
    }
    parts.join(", ")
}

fn render_hint(field: &str, value: &str) -> String {
    format!(
        "<a class=\"synapse-hint\" data-hint-for=\"{}\">Show hint</a><span class=\"synapse-hint-body\" hidden>{}</span>",
        html_escape_attr(field),
        value
    )
}

fn render_tts(filter: &str, value: &str) -> String {
    // Syntax: `tts en_US voices=Alice,Bob speed=1.2`
    let mut lang = "";
    let mut voices = "";
    let mut rate = "1";
    let rest = filter.strip_prefix("tts").unwrap_or("").trim();
    for tok in rest.split_whitespace() {
        if let Some(v) = tok.strip_prefix("voices=") {
            voices = v;
        } else if let Some(v) = tok.strip_prefix("speed=") {
            rate = v;
        } else if !tok.is_empty() {
            lang = tok;
        }
    }
    format!(
        "<span class=\"synapse-tts\" data-lang=\"{}\" data-voices=\"{}\" data-rate=\"{}\" data-text=\"{}\"></span>",
        html_escape_attr(lang),
        html_escape_attr(voices),
        html_escape_attr(rate),
        html_escape_attr(&strip_html(value)),
    )
}

enum FuriganaMode {
    Ruby,
    Reading,
    Base,
}

/// Converts `base[reading]` tokens to ruby markup, or extracts just one side.
fn furigana_transform(value: &str, mode: FuriganaMode) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"([^\s\[\]]+?)\[(.+?)\]").unwrap());
    re.replace_all(value, |caps: &regex::Captures<'_>| {
        let base = &caps[1];
        let reading = &caps[2];
        match mode {
            FuriganaMode::Ruby => format!("<ruby>{base}<rt>{reading}</rt></ruby>"),
            FuriganaMode::Reading => reading.to_string(),
            FuriganaMode::Base => base.to_string(),
        }
    })
    .into_owned()
}

fn strip_html(value: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"<[^>]*>").unwrap());
    re.replace_all(value, "").into_owned()
}

fn html_escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
