# Security policy

## Supported versions

Until the first stable release, security fixes target the latest `0.x` branch
and `main`. Older `0.x` releases may require users to upgrade rather than receive
backports. The policy will be revised when the project reaches `1.0`.

## Reporting a vulnerability

Please do not disclose a suspected vulnerability in a public issue, discussion,
pull request, or social channel.

Use GitHub's private vulnerability reporting for
`wintopic/TDesign-GPUI` when it is available under the repository Security tab.
If that interface is unavailable, contact the repository owner privately through
their GitHub profile and request a secure reporting channel. Include:

- affected commit, tag, crate, component, and platform;
- impact and realistic attack scenario;
- minimal reproduction or proof of concept;
- whether the issue involves SVG parsing, file selection/upload, HTTP requests,
  clipboard/IME data, upstream generation, or CI permissions;
- any suggested fix or embargo constraints.

Do not include real credentials, private files, or other users' data in a report.

## Response process

Maintainers will acknowledge a complete report as soon as practical, validate
scope and severity, coordinate a fix and release, and credit the reporter unless
anonymity is requested. Timelines depend on severity and maintainer availability;
please allow a reasonable private remediation period before public disclosure.

## Security boundaries

TDesign GPUI embeds sanitized upstream SVG assets and provides upload integration
points, but applications remain responsible for authorization, server-side file
validation, content limits, secret storage, TLS policy, and trust decisions for
custom `UploadBackend` implementations. GPUI and platform vulnerabilities should
also be reported to their respective upstream maintainers.
