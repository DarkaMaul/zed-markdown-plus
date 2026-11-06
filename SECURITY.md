# Security

Only the latest release is supported. Until 0.1.0 is published, there is no
published supported release.

Report security issues privately using GitHub's **Report a vulnerability** feature
when enabled. If it is unavailable, contact the maintainer through the
[DarkaMaul profile](https://github.com/DarkaMaul) to arrange a private report.
Do not put sensitive documents or exploit payloads in public issues.

Include the version, operating system, a minimal synthetic Markdown sample,
expected behavior, and observed behavior. Parser hangs, crashes, unexpected file
or network access, and document-triggered command execution are security-relevant.

The server processes editor-supplied text only. It does not follow URIs, render
HTML, execute code blocks, load user syntax definitions, or connect to a network.
Highlighting runs in a separate process with a deadline. Input limits and timeout
behavior are documented in the README. They mitigate resource exhaustion but are
not an OS sandbox or a hard memory quota. Zed's workspace settings and configured
server executable must be trusted.
