//! Bounded semantic highlighting for top-level fenced code using Syntect's bundled syntaxes.
//!
//! This deliberately uses only Syntect's compiled-in syntax set. It never loads user syntax
//! files, executes code, or fetches syntax definitions from the network.

use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd};
use std::ops::Range;
use std::sync::OnceLock;
use syntect::easy::ScopeRangeIterator;
use syntect::parsing::{ParseState, ScopeStack, SyntaxReference, SyntaxSet};

/// Token legend shared by the LSP capabilities and encoded lexer output.
pub const TOKEN_TYPES: [&str; 11] = [
    "keyword",
    "string",
    "comment",
    "number",
    "type",
    "function",
    "operator",
    "variable",
    "parameter",
    "enumMember",
    "regexp",
];

const MAX_HIGHLIGHT_BYTES: usize = 512 * 1024;
const MAX_LINE_BYTES: usize = 8 * 1024;
const MAX_DOCUMENT_LINES: usize = 100_000;
const MAX_HIGHLIGHT_LINES: usize = 12_000;
const MAX_TOKENS: usize = 20_000;
const MAX_INFO_BYTES: usize = 64;

static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();

/// Returns the LSP semantic-token integer array, delta encoded per LSP.
/// Unknown labels and fences nested in lists or blockquotes produce no tokens.
pub fn semantic_tokens(text: &str) -> Vec<u32> {
    if text.len() > super::MAX_DOCUMENT
        || text.bytes().filter(|&byte| byte == b'\n').count() >= MAX_DOCUMENT_LINES
    {
        return Vec::new();
    }

    let syntaxes = SYNTAXES.get_or_init(SyntaxSet::load_defaults_newlines);
    let blocks = top_level_fenced_blocks(text);
    let mut line_starts = vec![0usize];
    line_starts.extend(
        text.bytes()
            .enumerate()
            .filter_map(|(index, byte)| (byte == b'\n').then_some(index + 1)),
    );
    let mut highlighted_bytes = 0usize;
    let mut highlighted_lines = 0usize;
    let mut tokens = Vec::<(u32, u32, u32, u32)>::new();

    for (range, language) in blocks {
        let Some(syntax) = syntax_for_info(syntaxes, &language) else {
            continue;
        };
        let Some(code) = text.get(range.clone()) else {
            continue;
        };
        if highlighted_bytes.saturating_add(code.len()) > MAX_HIGHLIGHT_BYTES {
            break;
        }
        highlighted_bytes += code.len();

        let mut parser = ParseState::new(syntax);
        let mut scopes = ScopeStack::new();
        let mut absolute_offset = range.start;

        for raw_line in code.split_inclusive('\n') {
            let content = trim_line_ending(raw_line);
            if content.len() > MAX_LINE_BYTES {
                break;
            }
            highlighted_lines += 1;
            if highlighted_lines > MAX_HIGHLIGHT_LINES {
                return encode_tokens(tokens);
            }

            let mut parser_line = String::with_capacity(content.len() + 1);
            parser_line.push_str(content);
            parser_line.push('\n');
            let Ok(ops) = parser.parse_line(&parser_line, syntaxes) else {
                absolute_offset += raw_line.len();
                continue;
            };
            let (line, character) = point_for_offset(text, &line_starts, absolute_offset);

            for (span, op) in ScopeRangeIterator::new(&ops, &parser_line) {
                if scopes.apply(op).is_err() || span.start >= content.len() {
                    continue;
                }
                let end = span.end.min(content.len());
                if end <= span.start {
                    continue;
                }
                let Some(kind) = token_type(scopes.as_slice()) else {
                    continue;
                };
                let piece = &content[span.start..end];
                if piece.trim().is_empty() {
                    continue;
                }
                let start = character + utf16_len(&content[..span.start]);
                let length = utf16_len(piece);
                if length > 0 {
                    tokens.push((line, start, length, kind));
                    if tokens.len() >= MAX_TOKENS {
                        return encode_tokens(tokens);
                    }
                }
            }
            absolute_offset += raw_line.len();
        }
    }

    encode_tokens(tokens)
}

/// Finds source ranges emitted as text inside fenced code blocks at the document root.
/// Pulldown's parser handles fence boundaries, including unlabeled outer fences, so nested
/// backticks inside literal code are never mistaken for another block.
fn top_level_fenced_blocks(text: &str) -> Vec<(Range<usize>, String)> {
    let mut containers = 0usize;
    let mut current: Option<(bool, Option<String>, Option<Range<usize>>)> = None;
    let mut blocks = Vec::new();

    for (event, range) in Parser::new(text).into_offset_iter() {
        match event {
            Event::Start(Tag::BlockQuote(_))
            | Event::Start(Tag::List(_))
            | Event::Start(Tag::Item) => {
                containers = containers.saturating_add(1);
            }
            Event::End(TagEnd::BlockQuote(_))
            | Event::End(TagEnd::List(_))
            | Event::End(TagEnd::Item) => {
                containers = containers.saturating_sub(1);
            }
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) => {
                let root = containers == 0;
                let language = if root { language_label(&info) } else { None };
                current = Some((root, language, None));
            }
            Event::Text(_) => {
                if let Some((true, Some(_), Some(body))) = current.as_mut() {
                    if range.start >= body.end && range.end <= text.len() {
                        body.end = range.end;
                    }
                } else if let Some((true, Some(_), body)) = current.as_mut()
                    && range.end <= text.len()
                {
                    *body = Some(range);
                }
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some((true, Some(language), Some(body))) = current.take() {
                    if body.end >= body.start {
                        blocks.push((body, language));
                    }
                } else {
                    current = None;
                }
            }
            _ => {}
        }
    }
    blocks
}

fn language_label(info: &str) -> Option<String> {
    let label = info.split_whitespace().next()?;
    (!label.is_empty() && label.len() <= MAX_INFO_BYTES && label.is_ascii())
        .then(|| label.to_owned())
}

fn syntax_for_info<'a>(syntaxes: &'a SyntaxSet, info: &str) -> Option<&'a SyntaxReference> {
    let normalized = info.to_ascii_lowercase();
    let alias = match normalized.as_str() {
        "js" | "jsx" | "node" => "javascript",
        "ts" | "tsx" => "typescript",
        "py" => "python",
        "rs" => "rust",
        "sh" => "bash",
        "yml" => "yaml",
        "rb" => "ruby",
        "c++" | "cpp" | "cxx" => "C++",
        "c#" | "cs" => "C#",
        "kt" => "Kotlin",
        "md" => "markdown",
        _ => info,
    };
    syntaxes.find_syntax_by_token(alias)
}

fn token_type(scopes: &[syntect::parsing::Scope]) -> Option<u32> {
    let scope = scopes.last()?.build_string();
    let scope = scope
        .rsplit(' ')
        .next()
        .unwrap_or(&scope)
        .to_ascii_lowercase();
    let index = if scope.contains("comment") {
        2
    } else if scope.contains("string") || scope.contains("character") {
        1
    } else if scope.contains("regex") {
        10
    } else if scope.contains("constant.numeric") || scope.contains("number") {
        3
    } else if scope.contains("storage.type")
        || scope.contains("support.type")
        || scope.contains("entity.name.type")
    {
        4
    } else if scope.contains("entity.name.function") || scope.contains("support.function") {
        5
    } else if scope.contains("operator") {
        6
    } else if scope.contains("variable.parameter") {
        8
    } else if scope.contains("variable") {
        7
    } else if scope.contains("constant.language") || scope.contains("constant.character") {
        9
    } else if scope.contains("keyword") || scope.contains("storage.modifier") {
        0
    } else {
        return None;
    };
    Some(index)
}

fn point_for_offset(text: &str, line_starts: &[usize], byte_offset: usize) -> (u32, u32) {
    let offset = byte_offset.min(text.len());
    let line = line_starts
        .partition_point(|&start| start <= offset)
        .saturating_sub(1);
    (line as u32, utf16_len(&text[line_starts[line]..offset]))
}

fn trim_line_ending(line: &str) -> &str {
    let line = line.strip_suffix('\n').unwrap_or(line);
    line.strip_suffix('\r').unwrap_or(line)
}

fn utf16_len(text: &str) -> u32 {
    text.encode_utf16().count() as u32
}

fn encode_tokens(tokens: Vec<(u32, u32, u32, u32)>) -> Vec<u32> {
    let mut encoded = Vec::with_capacity(tokens.len() * 5);
    let (mut previous_line, mut previous_start) = (0u32, 0u32);
    for (line, start, length, token_type) in tokens {
        let line_delta = line.saturating_sub(previous_line);
        let start_delta = if line_delta == 0 {
            start.saturating_sub(previous_start)
        } else {
            start
        };
        encoded.extend([line_delta, start_delta, length, token_type, 0]);
        previous_line = line;
        previous_start = start;
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decoded(data: &[u32]) -> Vec<(u32, u32, u32, u32)> {
        data.chunks_exact(5)
            .scan((0, 0), |(line, start), token| {
                *line += token[0];
                *start = if token[0] == 0 {
                    *start + token[1]
                } else {
                    token[1]
                };
                Some((*line, *start, token[2], token[3]))
            })
            .collect()
    }

    #[test]
    fn highlights_java_keywords_types_and_numbers() {
        let tokens = decoded(&semantic_tokens(
            "```java\npublic class Demo { int count = 42; }\n```\n",
        ));
        assert!(tokens.iter().any(|token| token.3 == 0));
        assert!(tokens.iter().any(|token| token.3 == 3));
        assert!(tokens.iter().any(|token| token.3 == 4));
    }

    #[test]
    fn uses_utf16_columns_for_fenced_content() {
        let tokens = decoded(&semantic_tokens(
            "```java\nString s = \"😀\"; int value = 1;\n```\n",
        ));
        let number = tokens.iter().find(|token| token.3 == 3).unwrap();
        assert_eq!(number.0, 1);
        assert_eq!(number.1, 29);
    }

    #[test]
    fn ignores_nested_literal_fences_unknown_labels_and_containers() {
        let text = "````\n```java\nint literal = 1;\n```\n````\n\n> ```java\n> int quoted = 2;\n> ```\n\n- item\n  ```java\n  int listed = 3;\n  ```\n\n```unknown-language\nint ignored = 4;\n```\n";
        assert!(semantic_tokens(text).is_empty());
    }

    #[test]
    fn emits_no_tokens_for_fence_delimiters_or_surrounding_prose() {
        let text = "Intro text\n```java\nint value = 1;\n```\nTrailing prose\n";
        let tokens = decoded(&semantic_tokens(text));
        assert!(!tokens.is_empty());
        assert!(tokens.iter().all(|token| token.0 == 2));
    }
}
