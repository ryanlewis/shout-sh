// Cloudflare Worker: front shout.sh with HTTP support and CF-managed TLS.
// exe.dev forces HTTPS on its proxy; this Worker lets users hit `curl shout.sh/...`
// over plain HTTP without a 301. It forwards to the VM's exe.dev hostname so the
// exe.dev proxy treats the request as its own domain (no custom-domain cert dance).

const ORIGIN_HOST = "shout-sh.exe.xyz";

export default {
  async fetch(request) {
    const url = new URL(request.url);
    url.hostname = ORIGIN_HOST;
    url.protocol = "https:";
    url.port = "";

    const headers = new Headers(request.headers);
    headers.set("Host", ORIGIN_HOST);
    const clientIp = request.headers.get("CF-Connecting-IP");
    if (clientIp) headers.set("X-Forwarded-For", clientIp);
    headers.set("X-Forwarded-Host", new URL(request.url).host);
    headers.set("X-Forwarded-Proto", new URL(request.url).protocol.replace(":", ""));

    const upstream = new Request(url.toString(), {
      method: request.method,
      headers,
      body: request.body,
      redirect: "manual",
    });

    return fetch(upstream);
  },
};
