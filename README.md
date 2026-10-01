# Kreative Kompanion

A self-hosted, FOSS "Claude-like" companion: one orchestrator you talk to per
project, tasks that run on your own computers, local models doing the work and
a stronger model reviewing and teaching them. See `docs/architecture.md`.

## Status

First UI draft (`web/`), running against a fake server (`web/src/api/mock.ts`).
No real server or runner yet.

## Web app

Vanilla TypeScript, no framework. Built with esbuild; two small runtime
dependencies: `marked` (markdown) and `DOMPurify` (sanitising model output).

```sh
cd web
npm install
npm run dev        # http://localhost:5173
npm run typecheck
npm run build      # dist/
node build.mjs --single   # dist/preview.html, one self-contained file
```

Layout of `web/src`:

- `api/` types shared with the server, the `KompanionApi` interface, and the mock server
- `core/` small building blocks: escaped templates (`html`), sanitised markdown,
  a store that batches updates per animation frame, keyed list updates
- `views/` one file per screen part: connect, sidebar, conversation, tasks, machines, settings

Security rules the code follows: model text is never put into the page as raw
HTML (marked + DOMPurify, links forced to http(s) and `noopener`), templates
escape every value, a strict CSP with Trusted Types is set in `index.html`, and
approvals show the exact command, folder, machine and network targets.
