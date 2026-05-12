# usagiengine.com

Simple website that serves ../README.md as HTML with dark and light styles, as
well as syntax highlighting. Powered by Deno and hosted on Deno Deploy.

Development requires [Deno](https://deno.com) to be installed.

## Developing

Boot up the dev server at http://localhost:8008: `deno task dev`

## Install scripts

`install.sh` and `install.ps1` are served as `text/plain` from `/install.sh` and
`/install.ps1`. Both resolve the latest GitHub release, verify its SHA-256, and
install to `~/.usagi/bin/usagi` (or `%USERPROFILE%\.usagi\bin\usagi.exe` on
Windows).

```
curl -fsSL https://usagiengine.com/install.sh | sh
irm https://usagiengine.com/install.ps1 | iex
```

Override the install dir with `USAGI_INSTALL` / `$env:UsagiInstall`.
