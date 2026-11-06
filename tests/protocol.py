"""Exercise the built server over real stdio, including its isolated lexer worker."""
import json
from pathlib import Path
import subprocess

SERVER = Path(__file__).resolve().parents[1] / "target/release/markdown-folds-server"
URI = "file:///does-not-exist/never-read.md"


def message(method, params=None, request_id=None):
    value = {"jsonrpc": "2.0", "method": method, "params": params or {}}
    if request_id is not None:
        value["id"] = request_id
    body = json.dumps(value).encode()
    return f"Content-Length: {len(body)}\r\n\r\n".encode() + body


def exchange(messages):
    result = subprocess.run(
        [str(SERVER)], input=b"".join(messages), capture_output=True, timeout=10
    )
    assert result.returncode == 0, result.stderr
    output = result.stdout
    responses = {}
    while output:
        header, output = output.split(b"\r\n\r\n", 1)
        length = int(header.split(b":", 1)[1])
        response = json.loads(output[:length])
        output = output[length:]
        responses[response["id"]] = response
    return responses


document = {"textDocument": {"uri": URI}}
text = '# Parent\nbody\n## Child\n```java\nString s = "😀"; int n = 42;\n```\n# Next\nend\n'
responses = exchange([
    message("initialize", {"capabilities": {}}, 1),
    message("initialized"),
    message("textDocument/didOpen", {"textDocument": {
        "uri": URI, "languageId": "markdown", "version": 1, "text": text
    }}),
    message("textDocument/foldingRange", document, 2),
    message("textDocument/semanticTokens/full", document, 3),
    message("textDocument/didChange", {
        **document, "contentChanges": [{"text": "# Changed\nnew body\n"}]
    }),
    message("textDocument/foldingRange", document, 4),
    message("textDocument/semanticTokens/full", document, 5),
    # An invalid incremental update must discard stale document state.
    message("textDocument/didChange", {
        **document, "contentChanges": [{"range": {}, "text": "bad"}]
    }),
    message("textDocument/foldingRange", document, 6),
    message("textDocument/didClose", document),
    message("textDocument/semanticTokens/full", document, 7),
    message("unknown/request", {}, 8),
    message("shutdown", {}, 9),
    message("exit"),
])
assert responses[1]["result"]["capabilities"]["foldingRangeProvider"]
assert [(r["startLine"], r["endLine"]) for r in responses[2]["result"]] == [(0, 5), (2, 5), (6, 7)]
tokens = responses[3]["result"]["data"]
assert tokens and len(tokens) % 5 == 0
legend = responses[1]["result"]["capabilities"]["semanticTokensProvider"]["legend"]["tokenTypes"]
line = column = 0
for delta_line, delta_column, length, kind, modifiers in zip(*[iter(tokens)] * 5):
    line += delta_line
    column = column + delta_column if delta_line == 0 else delta_column
    assert line == 4 and length > 0 and kind < len(legend) and modifiers == 0
assert [(r["startLine"], r["endLine"]) for r in responses[4]["result"]] == [(0, 1)]
assert responses[5]["result"]["data"] == []
assert responses[6]["result"] == []
assert responses[7]["result"]["data"] == []
assert responses[8]["error"]["code"] == -32601
assert responses[9]["result"] is None

for frame in (
    b"Content-Length: 8388609\r\n\r\n",
    b"Content-Length: 2\r\nContent-Length: 2\r\n\r\n{}",
    b"X" * 4097,
):
    result = subprocess.run([str(SERVER)], input=frame, capture_output=True, timeout=3)
    assert result.returncode != 0
    assert not result.stdout
print("Protocol smoke tests passed: folds, worker tokens, updates, close, framing limits.")
