"""A tiny MCP server for tests.

stdio mode (default): newline-delimited JSON-RPC on stdin/stdout.
http mode (`http <port-file> <mode>`): serves Streamable HTTP at /mcp (mode `streamable`) or the
legacy SSE transport at /sse + /messages (mode `sse`; POST /mcp answers 405).
"""
import json
import os
import queue
import sys
import threading

TOOLS = [
    {
        "name": "echo",
        "description": "Echo the input text",
        "inputSchema": {
            "type": "object",
            "properties": {"text": {"type": "string"}},
            "required": ["text"],
        },
    },
    {"name": "image", "inputSchema": {"type": "object"}},
    {"name": "resource", "description": "Returns a resource block", "inputSchema": {"type": "object"}},
]


def handle(msg):
    """Return the response for a request, or None for a notification."""
    if "id" not in msg:
        return None
    method = msg.get("method")
    params = msg.get("params") or {}
    if method == "initialize":
        result = {
            "protocolVersion": params.get("protocolVersion", "2024-11-05"),
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "fake", "version": "0.0.1"},
            "echoedClientName": params.get("clientInfo", {}).get("name"),
        }
    elif method == "tools/list":
        result = {"tools": TOOLS}
    elif method == "tools/call":
        name = params.get("name")
        args = params.get("arguments") or {}
        if name == "echo":
            result = {"content": [{"type": "text", "text": "echo: " + str(args.get("text")), "annotations": {"audience": ["user"]}}]}
        elif name == "image":
            result = {"content": [{"type": "image", "data": "iVBORw0KGgo=", "mimeType": "image/png"}]}
        elif name == "resource":
            result = {"content": [{"type": "resource", "resource": {"uri": "file:///x", "text": "hi"}}]}
        else:
            return {"jsonrpc": "2.0", "id": msg["id"], "error": {"code": -32602, "message": "Unknown tool: " + str(name)}}
    elif method == "ping":
        result = {}
    else:
        return {"jsonrpc": "2.0", "id": msg["id"], "error": {"code": -32601, "message": "Method not found"}}
    return {"jsonrpc": "2.0", "id": msg["id"], "result": result}


def stdio():
    if os.environ.get("FAKE_MCP_REQUIRE_ENV") and os.environ.get("FAKE_MCP_REQUIRE_ENV") != "yes":
        sys.exit(3)
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        resp = handle(json.loads(line))
        if resp is not None:
            sys.stdout.write(json.dumps(resp) + "\n")
            sys.stdout.flush()


def http(port_file, mode):
    from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

    sessions = {}

    class Handler(BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.1"

        def log_message(self, *args):
            pass

        def _body(self):
            length = int(self.headers.get("Content-Length") or 0)
            return json.loads(self.rfile.read(length) or b"null")

        def _send(self, code, body=b"", ctype="application/json"):
            self.send_response(code)
            self.send_header("Content-Type", ctype)
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def do_POST(self):
            msg = self._body()  # always drain the body so keep-alive connections stay in sync
            if self.path.startswith("/mcp"):
                if mode != "streamable":
                    return self._send(405, b"")
                if self.headers.get("X-Test") != "1":
                    return self._send(401, b"")
                resp = handle(msg)
                if resp is None:
                    return self._send(202, b"")
                return self._send(200, json.dumps(resp).encode())
            if self.path.startswith("/messages"):
                sid = self.path.split("sessionId=")[-1]
                q = sessions.get(sid)
                if q is None:
                    return self._send(404, b"")
                resp = handle(msg)
                if resp is not None:
                    q.put(resp)
                return self._send(202, b"Accepted", "text/plain")
            self._send(404, b"")

        def do_GET(self):
            if self.path.startswith("/sse") and mode == "sse":
                if self.headers.get("X-Test") != "1":
                    return self._send(401, b"")
                sid = str(len(sessions) + 1)
                q = queue.Queue()
                sessions[sid] = q
                self.send_response(200)
                self.send_header("Content-Type", "text/event-stream")
                self.send_header("Cache-Control", "no-cache")
                self.end_headers()
                self.wfile.write(("event: endpoint\ndata: /messages?sessionId=%s\n\n" % sid).encode())
                self.wfile.flush()
                try:
                    while True:
                        resp = q.get()
                        self.wfile.write(("event: message\ndata: %s\n\n" % json.dumps(resp)).encode())
                        self.wfile.flush()
                except (BrokenPipeError, ConnectionResetError):
                    return
            if self.path.startswith("/mcp"):
                return self._send(405, b"")
            self._send(404, b"")

        def do_DELETE(self):
            self._send(405, b"")

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    with open(port_file + ".tmp", "w") as f:
        f.write(str(server.server_address[1]))
    os.replace(port_file + ".tmp", port_file)
    threading.Thread(target=lambda: (sys.stdin.read(), os._exit(0)), daemon=True).start()
    server.serve_forever()


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "http":
        http(sys.argv[2], sys.argv[3])
    else:
        stdio()
