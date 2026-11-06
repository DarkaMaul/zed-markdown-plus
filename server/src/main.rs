//! A local stdio LSP for Markdown section folds and bounded fenced-code colors.
#![forbid(unsafe_code)]
#![deny(missing_docs, rustdoc::broken_intra_doc_links)]

use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Read, Write};

mod highlight;
mod worker;

const MAX_FRAME: usize = 8 * 1024 * 1024;
const MAX_HEADER: usize = 4096;
const MAX_DOCUMENT: usize = 2 * 1024 * 1024;
const MAX_OPEN_BYTES: usize = 16 * 1024 * 1024;
const MAX_OPEN_DOCUMENTS: usize = 256;
const MAX_URI: usize = 4096;

fn main() -> io::Result<()> {
    if std::env::args().nth(1).as_deref() == Some("--highlight-worker") {
        return worker::run();
    }
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = stdout.lock();
    let mut docs: HashMap<String, String> = HashMap::new();
    let mut open_bytes = 0usize;

    while let Some(message) = read_message(&mut reader)? {
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            continue;
        };
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        let id = message.get("id").cloned();
        let mut response = None;

        match method {
            "initialize" => {
                if let Some(id) = id {
                    response = Some(json!({"jsonrpc":"2.0", "id":id, "result":{
                        "capabilities": {
                            "textDocumentSync": 1,
                            "foldingRangeProvider": true,
                            "semanticTokensProvider": {
                                "legend": {"tokenTypes": highlight::TOKEN_TYPES, "tokenModifiers": []},
                                "full": true
                            }
                        },
                        "serverInfo": {"name":"markdown-folds-server", "version":"0.1.0"}
                    }}));
                }
            }
            "initialized" => {}
            "shutdown" => {
                if let Some(id) = id {
                    response = Some(json!({"jsonrpc":"2.0", "id":id, "result":null}));
                }
            }
            "exit" => break,
            "textDocument/didOpen" => {
                if let (Some(uri), Some(text)) = (
                    document_uri(&params),
                    params.pointer("/textDocument/text").and_then(Value::as_str),
                ) {
                    set_document(&mut docs, &mut open_bytes, uri, text);
                }
            }
            "textDocument/didChange" => {
                if let Some(uri) = params.pointer("/textDocument/uri").and_then(Value::as_str) {
                    let changes = params.get("contentChanges").and_then(Value::as_array);
                    let full_text = changes.and_then(|changes| {
                        changes.iter().try_fold(None, |_, change| {
                            if change.get("range").is_some() || change.get("rangeLength").is_some()
                            {
                                None
                            } else {
                                change.get("text").and_then(Value::as_str).map(Some)
                            }
                        })
                    });
                    if let Some(Some(text)) = full_text {
                        set_document(&mut docs, &mut open_bytes, uri, text);
                    } else {
                        remove_document(&mut docs, &mut open_bytes, uri);
                    }
                }
            }
            "textDocument/didClose" => {
                if let Some(uri) = document_uri(&params) {
                    remove_document(&mut docs, &mut open_bytes, uri);
                }
            }
            "textDocument/foldingRange" => {
                if let Some(id) = id {
                    let ranges = document_uri(&params)
                        .and_then(|uri| docs.get(uri))
                        .map(|text| folding_ranges(text))
                        .unwrap_or_default();
                    response = Some(json!({"jsonrpc":"2.0", "id":id, "result":ranges}));
                }
            }
            "textDocument/semanticTokens/full" => {
                if let Some(id) = id {
                    let tokens = document_uri(&params)
                        .and_then(|uri| docs.get(uri))
                        .map(|text| worker::tokens(text).unwrap_or_default())
                        .unwrap_or_default();
                    response = Some(json!({"jsonrpc":"2.0", "id":id, "result":{"data":tokens}}));
                }
            }
            _ => {
                if let Some(id) = id {
                    response = Some(
                        json!({"jsonrpc":"2.0", "id":id, "error":{"code":-32601,"message":"Method not found"}}),
                    );
                }
            }
        }

        if let Some(message) = response {
            write_message(&mut writer, &message)?;
        }
    }
    Ok(())
}

fn document_uri(params: &Value) -> Option<&str> {
    params.pointer("/textDocument/uri").and_then(Value::as_str)
}

fn set_document(docs: &mut HashMap<String, String>, open_bytes: &mut usize, uri: &str, text: &str) {
    remove_document(docs, open_bytes, uri);
    if uri.len() <= MAX_URI
        && text.len() <= MAX_DOCUMENT
        && docs.len() < MAX_OPEN_DOCUMENTS
        && open_bytes.saturating_add(text.len()) <= MAX_OPEN_BYTES
    {
        *open_bytes += text.len();
        docs.insert(uri.to_owned(), text.to_owned());
    }
}

fn remove_document(docs: &mut HashMap<String, String>, open_bytes: &mut usize, uri: &str) {
    if let Some(old) = docs.remove(uri) {
        *open_bytes = open_bytes.saturating_sub(old.len());
    }
}

fn folding_ranges(text: &str) -> Vec<Value> {
    if text.len() > MAX_DOCUMENT {
        return Vec::new();
    }
    let mut line_starts = vec![0usize];
    line_starts.extend(text.match_indices('\n').map(|(index, _)| index + 1));
    let mut headings = Vec::new();
    let mut container_depth = 0usize;
    for (event, range) in Parser::new(text).into_offset_iter() {
        match event {
            Event::Start(Tag::BlockQuote(_) | Tag::List(_)) => container_depth += 1,
            Event::End(TagEnd::BlockQuote(_) | TagEnd::List(_)) => {
                container_depth = container_depth.saturating_sub(1);
            }
            Event::Start(Tag::Heading { level, .. }) if container_depth == 0 => {
                let line = line_starts
                    .partition_point(|&offset| offset <= range.start)
                    .saturating_sub(1);
                headings.push((line, heading_rank(level)));
            }
            _ => {}
        }
    }
    let mut ranges = vec![None; headings.len()];
    let mut open: Vec<(usize, usize, u8)> = Vec::new();
    for (index, (line, rank)) in headings.iter().copied().enumerate() {
        while open
            .last()
            .is_some_and(|(_, _, open_rank)| *open_rank >= rank)
        {
            let (start, start_index, _) = open.pop().unwrap();
            let end = line.saturating_sub(1);
            if end > start {
                ranges[start_index] =
                    Some(json!({"startLine":start, "endLine":end, "kind":"region"}));
            }
        }
        open.push((line, index, rank));
    }
    let last_line = line_starts.len().saturating_sub(1) - usize::from(text.ends_with('\n'));
    while let Some((start, index, _)) = open.pop() {
        let end = last_line;
        if end > start {
            ranges[index] = Some(json!({"startLine":start, "endLine":end, "kind":"region"}));
        }
    }
    ranges.into_iter().flatten().collect()
}

fn heading_rank(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn read_message(reader: &mut impl BufRead) -> io::Result<Option<Value>> {
    let mut content_length = None;
    let mut total_header = 0;
    loop {
        let mut line = Vec::new();
        let n = reader
            .take((MAX_HEADER + 1) as u64)
            .read_until(b'\n', &mut line)?;
        if n == 0 {
            return Ok(None);
        }
        total_header += n;
        if total_header > MAX_HEADER {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "header too large",
            ));
        }
        if line == b"\r\n" || line == b"\n" {
            break;
        }
        let header = std::str::from_utf8(&line)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid header"))?;
        if let Some((name, value)) = header.trim_end_matches(['\r', '\n']).split_once(':')
            && name.eq_ignore_ascii_case("Content-Length")
        {
            if content_length.is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "duplicate Content-Length",
                ));
            }
            content_length = Some(value.trim().parse::<usize>().map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "invalid Content-Length")
            })?);
        }
    }
    let length = content_length
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing Content-Length"))?;
    if length > MAX_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "frame too large",
        ));
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    let message = serde_json::from_slice(&body)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid JSON"))?;
    Ok(Some(message))
}

fn write_message(writer: &mut impl Write, message: &Value) -> io::Result<()> {
    let body = serde_json::to_vec(message).map_err(io::Error::other)?;
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(&body)?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_heading_sections_and_preserves_heading_lines() {
        let text = "# One\nbody\n## Two\nmore\n# Three\nend\n";
        assert_eq!(
            folding_ranges(text),
            vec![
                json!({"startLine":0,"endLine":3,"kind":"region"}),
                json!({"startLine":2,"endLine":3,"kind":"region"}),
                json!({"startLine":4,"endLine":5,"kind":"region"}),
            ]
        );
    }

    #[test]
    fn supports_setext_and_ignores_fenced_fake_headings() {
        let text = "Title\n=====\nbody\n```md\n# attacker\n```\nNext\n----\nlast\n";
        assert_eq!(
            folding_ranges(text),
            vec![
                json!({"startLine":0,"endLine":8,"kind":"region"}),
                json!({"startLine":6,"endLine":8,"kind":"region"}),
            ]
        );
    }

    #[test]
    fn ignores_heading_like_text_in_code_and_blockquotes() {
        let text = "    # code\n> # Quote\ntext\n";
        assert_eq!(folding_ranges(text), Vec::<Value>::new());
    }

    #[test]
    fn bounds_document_and_stored_bytes() {
        let mut docs = HashMap::new();
        let mut bytes = 0;
        set_document(&mut docs, &mut bytes, "a", &"x".repeat(MAX_DOCUMENT + 1));
        assert_eq!(docs.len(), 0);
        set_document(&mut docs, &mut bytes, "a", "hello");
        assert_eq!(bytes, 5);
        set_document(&mut docs, &mut bytes, "a", "world!");
        assert_eq!(bytes, 6);
    }
}
