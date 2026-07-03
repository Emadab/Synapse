//! Cloze-deletion rendering: `{{cN::text::hint}}` markers, the `cloze-only:`
//! filter, and the image-occlusion passthrough (occlusion shapes are stored
//! as cloze markers whose text begins with `image-occlusion:`).

use std::sync::OnceLock;

use regex::Regex;

fn cloze_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)\{\{c(\d+)::(.*?)(?:::(.*?))?\}\}").unwrap())
}

/// Replace `{{cN::text::hint}}` markers. On the question, the active cloze is
/// hidden (`[hint]` or `[...]`) and the rest show their text; on the answer
/// all are revealed, with the active one highlighted.
pub fn process_cloze(value: &str, active: u16, reveal: bool, occlusion_mode: &str) -> String {
    cloze_re()
        .replace_all(value, |caps: &regex::Captures<'_>| {
            let n: u16 = caps[1].parse().unwrap_or(0);
            let text = &caps[2];
            let hint = caps.get(3).map(|m| m.as_str()).filter(|h| !h.is_empty());

            if let Some(shape) = text.strip_prefix("image-occlusion:") {
                return render_occlusion_shape(n, shape, active, reveal, occlusion_mode);
            }

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

/// `{{cloze-only:Field}}` — like `cloze`, but strips all text outside cloze
/// markers, leaving only the active cloze's content (used to feed TTS or
/// isolated review of just the deleted text).
pub fn process_cloze_only(value: &str, active: u16, reveal: bool) -> String {
    let mut out = String::new();
    for caps in cloze_re().captures_iter(value) {
        let n: u16 = caps[1].parse().unwrap_or(0);
        if n != active {
            continue;
        }
        let text = &caps[2];
        let hint = caps.get(3).map(|m| m.as_str()).filter(|h| !h.is_empty());
        if reveal {
            out.push_str(&format!("<span class=\"cloze\">{text}</span>"));
        } else {
            match hint {
                Some(h) => out.push_str(&format!("<span class=\"cloze\">[{h}]</span>")),
                None => out.push_str("<span class=\"cloze\">[...]</span>"),
            }
        }
    }
    out
}

/// One occlusion shape → a positioned overlay div. `shape` is
/// `rect:left=.1:top=.2:width=.3:height=.15` (or `ellipse:...`).
///
/// `occlusion_mode` follows Anki's Image Occlusion conventions:
/// - `"hideAllGuessOne"` — every shape is hidden on the question side
///   (regardless of which one is "active" for this card); all reveal on the
///   answer side.
/// - anything else (default `"hideOneGuessOne"`) — only the active shape is
///   hidden on the question side; the rest stay visible as positional hints.
fn render_occlusion_shape(
    ord: u16,
    shape: &str,
    active: u16,
    reveal: bool,
    occlusion_mode: &str,
) -> String {
    let mut kind = "rect";
    let mut style = String::new();
    for (i, tok) in shape.split(':').enumerate() {
        if i == 0 {
            kind = tok;
            continue;
        }
        if let Some((k, v)) = tok.split_once('=') {
            let prop = match k {
                "left" => "left",
                "top" => "top",
                "width" => "width",
                "height" => "height",
                _ => continue,
            };
            style.push_str(&format!("--io-{prop}:{v};"));
        }
    }
    let is_active = ord == active;
    let hide_all = occlusion_mode == "hideAllGuessOne";
    let revealed = if reveal {
        true
    } else if hide_all {
        false
    } else {
        !is_active
    };
    format!(
        "<div class=\"synapse-io-shape synapse-io-{kind}\" data-ord=\"{ord}\" data-active=\"{is_active}\" data-revealed=\"{revealed}\" style=\"{style}\"></div>",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloze_only_keeps_just_active_text() {
        let v = "{{c1::Paris}} is in {{c2::France}}";
        assert_eq!(
            process_cloze_only(v, 1, true),
            "<span class=\"cloze\">Paris</span>"
        );
        assert_eq!(
            process_cloze_only(v, 1, false),
            "<span class=\"cloze\">[...]</span>"
        );
    }

    #[test]
    fn occlusion_shape_renders_positioned_div() {
        let v = "{{c1::image-occlusion:rect:left=.1:top=.2:width=.3:height=.15}}";
        let html = process_cloze(v, 1, false, "");
        assert!(html.contains("synapse-io-shape"));
        assert!(html.contains("data-active=\"true\""));
        assert!(html.contains("--io-left:.1;"));
    }

    #[test]
    fn hide_one_guess_one_only_hides_the_active_shape() {
        let v = "{{c1::image-occlusion:rect:left=0:top=0:width=.1:height=.1}}{{c2::image-occlusion:rect:left=.2:top=.2:width=.1:height=.1}}";
        let html = process_cloze(v, 1, false, "hideOneGuessOne");
        // shape 1 (active) hidden, shape 2 revealed.
        assert!(html.contains("data-ord=\"1\" data-active=\"true\" data-revealed=\"false\""));
        assert!(html.contains("data-ord=\"2\" data-active=\"false\" data-revealed=\"true\""));
    }

    #[test]
    fn hide_all_guess_one_hides_every_shape_on_the_question() {
        let v = "{{c1::image-occlusion:rect:left=0:top=0:width=.1:height=.1}}{{c2::image-occlusion:rect:left=.2:top=.2:width=.1:height=.1}}";
        let question = process_cloze(v, 1, false, "hideAllGuessOne");
        assert!(question.contains("data-ord=\"1\" data-active=\"true\" data-revealed=\"false\""));
        assert!(question.contains("data-ord=\"2\" data-active=\"false\" data-revealed=\"false\""));

        let answer = process_cloze(v, 1, true, "hideAllGuessOne");
        assert!(answer.contains("data-ord=\"1\" data-active=\"true\" data-revealed=\"true\""));
        assert!(answer.contains("data-ord=\"2\" data-active=\"false\" data-revealed=\"true\""));
    }
}
