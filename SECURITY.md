# Security policy

OpenDraft is a young, one-maintainer project. Reports are read and answered on a best-effort basis; there is no bug bounty.

## Reporting a vulnerability

Please do **not** open a public issue for a security problem.

- Use GitHub's private reporting: open the repository's **Security** tab and choose **Report a vulnerability**. Only you and the maintainer can see it.
- Or email **contact@openengineering.uk** with "OpenDraft security" in the subject.

Please include the version (shown under the title on the Home page), your operating system, and what you found. If a drawing file triggers it, send the smallest file that does, and say which format it is (`.odx`, `.dxf` or `.dwg`). Remove any private information from it first.

You will get a reply to say the report arrived. Once a fix is released, the report can be made public, and you will be credited if you wish. Please give the fix a reasonable chance to reach users before you publish details.

## What is in scope

OpenDraft opens files it did not make and talks to other programs, so these matter most:

- Reading drawings: `.odx`, DXF and DWG files that crash the program, use unbounded memory or time, or write outside the intended place.
- The assistant interface (`opendraft-mcp` and the endpoint the app opens when you turn it on): anything that lets a program or user reach drawings or files you have not allowed.
- The update check (it contacts GitHub only after you allow it) and the installers and packages.

A bug that only produces a wrong drawing or a wrong measurement is not a security problem; report it as an ordinary [bug](https://github.com/JamesHodgkins/OpenDraft/issues/new/choose).

## Supported versions

Only the latest release is supported. Fixes are made on `main` and released as a new version.
