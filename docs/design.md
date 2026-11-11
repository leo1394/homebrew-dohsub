# Inputs, runtime state and scheduling / 输入与运行说明

Examples use **synthetic** passwords. Supply your actual full configuration locally, never in a chat.
示例密码为虚构值。真实完整配置只在本机使用。

## QX text: nodes.txt

One complete Shadowsocks node per line; blank lines and `#` / `;` comments are ignored.
每行一个完整节点，不是只提供域名。

```text
# synthetic examples / 虚构示例
shadowsocks=relay.example.com:8443, method=chacha20-ietf-poly1305, password=example-only-change-me, fast-open=false, udp-relay=false, tag=Example-US
shadowsocks=relay-sg.example.com:443, method=aes-128-gcm, password=another-example-only, tag=Example-SG
```

```bash
chmod 600 "$HOME/Downloads/nodes.txt"
dohsub import --file "$HOME/Downloads/nodes.txt" -y
```

QX options are preserved in order, including supported TLS/obfuscation fields. Duplicate keys, controls/newlines, and values containing commas are rejected because this line format cannot safely represent them. The generated ` · AutoIP` suffix is stripped on import to avoid repetition. Unicode tags are supported. Hostnames are ASCII DNS names (use punycode for IDNs).

`--domain relay.example.com` replaces the host in **one** imported node, useful when a previously tested QX node contains a literal IP. All method/password/port parameters still need to be supplied. Public literal IPv4 inputs are retained without DNS lookups; private/reserved IPs and IPv6 are rejected.

## ss:// links

SIP002 base64 userinfo and older whole-body base64 links are supported, with percent-decoded tags. Example (synthetic `aes-128-gcm:fixture-only` credentials):

```text
ss://YWVzLTEyOC1nY206Zml4dHVyZS1vbmx5@relay.example.com:443#Example-US
```

SS plugin query parameters are rejected; import full QX text when you need those options. This version does not implement SSR, VMess, Trojan or other node protocols.

## Exported servers JSON / 节点 JSON 数组

A node file is a top-level JSON array, independent of the exporting application and file name. Relevant fields:

```json
[
  {
    "hostname": "relay.example.com",
    "server_port": 8443,
    "port": 8443,
    "encryption": "chacha20-ietf-poly1305",
    "password": "example-only-change-me",
    "alias": "Example-US",
    "enable": 1
  }
]
```

```bash
chmod 600 "$HOME/Downloads/servers.json"
dohsub import --file "$HOME/Downloads/servers.json" -y
```

`hostname`, `encryption`, `password` and a valid `server_port` (fallback `port`) are required. If both port fields exist, they must agree. `alias` becomes the tag, falling back to hostname:port. Disabled items (`enable: 0/false`) are reported and skipped. Missing enable is allowed. Provider-specific metadata such as category is ignored; it does not determine protocol or override required Shadowsocks fields. Default QX options: `fast-open=false, udp-relay=false`.

Imports are merge/upsert operations by exact tag, not complete replacement. A tag found multiple times in one input is rejected after the first. Every import/remove/restore with accepted nodes saves a one-level previous snapshot. Passwords and raw malformed lines are not printed. Each rejected item uses its item number and a safe reason. Use `chmod 600 FILE` before import; symlinks and files not owned by the current user are refused.

## DoH protocol and SS identity

The endpoint URL path can be `/dns-query` or any other user-configured path. The path alone does not determine whether it is standard DoH. This implementation uses:

1. Encode a DNS A/IN wire query, with DNS message ID 0.
2. HTTPS GET `CONFIGURED_URL?dns=BASE64URL_WITHOUT_PADDING`, preserving other query parameters.
3. Send `Accept: application/dns-message`, require a successful HTTPS status, parse the binary DNS message and validate its ID/question/type/class.
4. Follow CNAMEs within the answer; use only matching public A answers and their minimum TTL.

HTTPS validates the **DoH server's TLS identity**, including IP SAN validation for an IP-literal URL. No `--insecure` is used. Outgoing DoH bypasses environment proxy variables to avoid depending on the subscription it is generating. `--interface` can bind DoH requests to a physical network interface when needed. User curl configuration is disabled for predictable TLS/timeouts.

Ordinary Shadowsocks AEAD authenticates encrypted data using the shared secret; it does **not** perform HTTPS-style hostname certificate verification. Replacing the node hostname with its IP does not remove a certificate check that plain SS would have performed. If a node has an extra TLS/obfuscation layer, preserve its distinct server-name options; do not infer them from the replaced IP. This service does not connect to the proxy or prove it works. Confirm actual latency/traffic in QX.

## Persistent state and atomicity

| File in `~/.dohsub/` | Purpose |
| --- | --- |
| `config.json` | Cached DoH URL, port and optional interface; next start/restart |
| `nodes.json` | Staged node configuration |
| `nodes.previous.json` | One modification back; restore swaps snapshots |
| `runtime.json` | Active node/config snapshot |
| `dns-cache.json` | IP/TTL plus per-host retry deadlines, scoped to DoH URL |
| `token` | 256-bit random subscription token |
| `control.sock` | Mode-600 Unix socket for CLI control; never sends passwords |
| `daemon.lock`, `nodes.lock`, `config.lock` | Process and modification locks |
| `service.log` | Private startup/error messages; no HTTP access URL logging |

State replacement uses same-directory exclusive temporary files, file fsync and rename. It is atomic for an individual file; a pair of backup/current writes is not a cross-file transaction. Backup is written first. The user's OS account remains the security boundary; this is not encrypted credential storage. Parent-path ownership is trusted. Do not place state in untrusted shared directory trees.

`start` reserves and inherits the actual listening descriptor, avoiding a check-then-bind race. It holds an inherited exclusive process lock, detaches with setsid, verifies the new daemon through private IPC, then caches the selected configuration. An explicit occupied port fails, even if another port is free. Startup requires a syntactically valid DoH URL and a valid DNS response; negative responses count as reachable. An imported hostname without an answer can therefore be retained for retries. Endpoints and node credentials are supplied explicitly by the user.

`restart` validates the new DoH before stopping the current process, then starts with cached settings. A later bind/start failure can still leave it stopped; it is not a zero-downtime rolling restart. Configuration flags passed to a failed start are not cached. Optional `--import` can persist staged nodes even if startup later fails.

A running service owns its active snapshot. Import without confirmation changes only `nodes.json`. `refresh` loads it and bypasses backoff for one round. `remove`/`restore` load automatically. `list` shows staged tags and whether the corresponding active tag is published, plus active counts; staged parameter changes under the same tag can differ until loaded.

## Scheduling and publication

- Success: honor DNS TTL clamped to **5–3600 seconds**; one query per unique hostname when due. Retain the previous IP if still in the current A answers.
- NXDOMAIN/no usable public A: **15 min → 30 min → 1 h → 2 h → 4 h → 6 h cap**.
- HTTPS/network/SERVFAIL/transient error: **30 s → 60 s → … → 15 min cap**.
- Retry delays have 80–100% jitter; deadlines survive restarts. Changing the DoH URL resets resolution state. Removed hosts are pruned.
- `refresh` bypasses the deadline once; success resets failure count. Idle waiting uses a condition variable, not a polling loop. DNS requests are sequential and bounded (3-second connect, 8-second total).
- A cached public address can be served for at most **15 minutes after expiry**, while retrying. There is no unlimited stale-node publication.
- Subscription reads use only the active snapshot and cache. Partially resolved sets return HTTP 200 with available nodes; all unresolved/empty returns 503. Clients control whether to retain the last subscription after errors.
- HTTP `/health` exposes safe tags/counts/retry metadata locally. The token-bearing `/sub/TOKEN/nodes.conf` contains credentials and must remain private. Foreign Host/Origin requests are rejected; this does not protect against other processes running as the same user.
- Automatic QX node ranking/failover is outside this service. QX's own policy groups decide which imported node is selected. DNS availability alone cannot drive reliable proxy failover.

## Verification scope

No live subscription or real provider credentials are used in tests. Unit and isolated behavioral tests exercise DNS parsing, duplicate imports, staging/confirmation, retry persistence, process lifetime, HTTP controls, port collision, installer failure preservation and mocked release upgrade. Current platform evidence is macOS arm64 only; full Homebrew installation, Linux, remote release downloads, and real QX import remain unverified until explicitly tested. See `docs/releasing.md` before publishing.
