# Security

Found something that looks like a vulnerability in shout.sh? Email
**ryan@rlew.io** with details and a way to reproduce it. Please don't open
a public issue for unpatched security problems.

I'll acknowledge within a few days. This is a small personal project, so
expect best-effort timing rather than an SLA — but real issues will get
real attention.

## Scope

In scope:

- The HTTP service running at `shout.sh`, which is the Cloudflare Worker
  in `shout-worker/`.
- The WebAssembly playground served from `shout.sh` (built from
  `shout-wasm` and `web/`).

Out of scope:

- Reports about the upstream [cfonts] library — please file those with
  cfonts directly.
- Volumetric DoS or generic load-test results. The service is a tiny
  free toy. Enough load will hit its CPU and plan limits, and that's not
  a vulnerability.
- Findings that boil down to "URLs you `curl` are visible in your shell
  history / proxy logs" — that's how curl works and the privacy page
  says so.

[cfonts]: https://github.com/dominikwilkowski/cfonts
