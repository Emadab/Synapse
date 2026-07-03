//! Tokenizes and parses Anki template syntax (`{{Field}}`, `{{#F}}…{{/F}}`,
//! `{{^F}}…{{/F}}`) into an AST. Unlike a single regex pass, this correctly
//! handles nested same-name conditional sections and degrades unmatched
//! `{{/…}}` closers to literal text instead of eating surrounding content.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    Text(String),
    Replacement {
        filters: Vec<String>,
        field: String,
    },
    Section {
        negate: bool,
        name: String,
        body: Vec<Node>,
    },
}

enum Token<'a> {
    Text(&'a str),
    Open { negate: bool, name: &'a str },
    Close { name: &'a str },
    Replacement { inner: &'a str },
}

pub fn parse(template: &str) -> Vec<Node> {
    let tokens = tokenize(template);
    let mut pos = 0;
    parse_nodes(&tokens, &mut pos, None)
}

fn tokenize(template: &str) -> Vec<Token<'_>> {
    let mut tokens = Vec::new();
    let mut i = 0;
    let mut text_start = 0;
    while i < template.len() {
        if template.as_bytes()[i] == b'{'
            && i + 1 < template.len()
            && template.as_bytes()[i + 1] == b'{'
        {
            if let Some(rel_end) = template[i..].find("}}") {
                let close_idx = i + rel_end;
                if text_start < i {
                    tokens.push(Token::Text(&template[text_start..i]));
                }
                let inner = &template[i + 2..close_idx];
                let inner_trim = inner.trim();
                if let Some(name) = inner_trim.strip_prefix('#') {
                    tokens.push(Token::Open {
                        negate: false,
                        name: name.trim(),
                    });
                } else if let Some(name) = inner_trim.strip_prefix('^') {
                    tokens.push(Token::Open {
                        negate: true,
                        name: name.trim(),
                    });
                } else if let Some(name) = inner_trim.strip_prefix('/') {
                    tokens.push(Token::Close { name: name.trim() });
                } else {
                    tokens.push(Token::Replacement { inner });
                }
                i = close_idx + 2;
                text_start = i;
                continue;
            }
        }
        i += 1;
    }
    if text_start < template.len() {
        tokens.push(Token::Text(&template[text_start..]));
    }
    tokens
}

/// Parses a run of nodes, stopping (without consuming) when it meets a close
/// tag matching `closing`. Called recursively for section bodies so nested
/// same-name sections resolve independently at each depth.
fn parse_nodes<'a>(tokens: &[Token<'a>], pos: &mut usize, closing: Option<&str>) -> Vec<Node> {
    let mut nodes = Vec::new();
    while *pos < tokens.len() {
        match &tokens[*pos] {
            Token::Text(t) => {
                nodes.push(Node::Text(t.to_string()));
                *pos += 1;
            }
            Token::Replacement { inner } => {
                let mut parts: Vec<&str> = inner.split(':').collect();
                let field = parts.pop().unwrap_or("").trim().to_string();
                let filters = parts.into_iter().map(|f| f.trim().to_string()).collect();
                nodes.push(Node::Replacement { filters, field });
                *pos += 1;
            }
            Token::Open { negate, name } => {
                let negate = *negate;
                let name = name.to_string();
                *pos += 1;
                let body = parse_nodes(tokens, pos, Some(&name));
                if *pos < tokens.len() {
                    if let Token::Close { name: close_name } = &tokens[*pos] {
                        if *close_name == name {
                            *pos += 1;
                        }
                    }
                }
                nodes.push(Node::Section { negate, name, body });
            }
            Token::Close { name } => {
                if closing == Some(*name) {
                    return nodes;
                }
                // Unmatched closer — degrade to literal text rather than
                // silently swallowing surrounding content.
                nodes.push(Node::Text(format!("{{{{/{name}}}}}")));
                *pos += 1;
            }
        }
    }
    nodes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_replacement_and_text() {
        let nodes = parse("hi {{Front}}!");
        assert_eq!(
            nodes,
            vec![
                Node::Text("hi ".into()),
                Node::Replacement {
                    filters: vec![],
                    field: "Front".into()
                },
                Node::Text("!".into()),
            ]
        );
    }

    #[test]
    fn nested_same_name_sections() {
        let nodes = parse("{{#A}}outer{{#A}}inner{{/A}}rest{{/A}}");
        assert_eq!(
            nodes,
            vec![Node::Section {
                negate: false,
                name: "A".into(),
                body: vec![
                    Node::Text("outer".into()),
                    Node::Section {
                        negate: false,
                        name: "A".into(),
                        body: vec![Node::Text("inner".into())],
                    },
                    Node::Text("rest".into()),
                ],
            }],
        );
    }

    #[test]
    fn unmatched_close_degrades_to_text() {
        let nodes = parse("a{{/Stray}}b");
        assert_eq!(
            nodes,
            vec![
                Node::Text("a".into()),
                Node::Text("{{/Stray}}".into()),
                Node::Text("b".into()),
            ]
        );
    }

    #[test]
    fn chained_filters_parsed_in_order() {
        let nodes = parse("{{text:cloze:Text}}");
        assert_eq!(
            nodes,
            vec![Node::Replacement {
                filters: vec!["text".into(), "cloze".into()],
                field: "Text".into(),
            }]
        );
    }
}
