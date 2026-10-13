// Local development mock only. No ChatGPT login or upstream requests.
const fixture = Bun.file(new URL("../config/usage-mock.json", import.meta.url));
const server = Bun.serve({
  hostname: "127.0.0.1",
  port: 8787,
  async fetch(request) {
    if (new URL(request.url).pathname !== "/usage") {
      return new Response("Not found", { status: 404 });
    }
    if (request.method !== "GET") {
      return new Response("Method not allowed", { status: 405 });
    }
    try {
      // Read on every request so editing the fixture changes the next refresh.
      return Response.json(await fixture.json(), {
        headers: { "Cache-Control": "no-store" },
      });
    } catch {
      return new Response("Mock fixture unavailable", { status: 503 });
    }
  },
});
console.log(`Mock usage endpoint: ${server.url}usage (sample data only)`);
