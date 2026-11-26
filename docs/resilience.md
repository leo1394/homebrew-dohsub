# DNS recovery and relay health / DNS 恢复与中继状态

DoH transport errors receive at most three attempts within a total 10-second
budget. Each TLS/connect attempt retains its 3-second limit; HTTP errors and
malformed DNS responses are not blindly retried. Configuration secrets are never
included in error output.

Each relay refreshes DNS after 70–85% of its TTL, staggered by its local port.
Concurrent cold requests share one resolver operation. Background refresh does
not block connections using valid cached addresses. Failed refreshes back off
from 2 to 30 seconds. No address is used after its original TTL, and there is no
system-DNS or system-route fallback. Persistent DoH outages can still make nodes
unavailable; retries cannot repair the remote TLS path.

`dohsub status` and `/health` expose `relay_health` keyed by local listener port:

- `dns_state`: pending, refreshing, ready or retrying.
- `dns_expires`: Unix timestamp; zero means no verified DNS answer yet.
- `retry_at`: Unix timestamp for the next resolver attempt.
- `upstream`: last observed outcome, including unverified, dns_failed,
  interface_failed, tcp_failed, tcp_connected, handshake_failed, io_failed,
  verified or engine_exited.
- `last_success`: Unix timestamp of the last authenticated upstream response;
  zero means no successful response has been observed.

`verified` is recorded only after the protocol decoder returns response bytes.
TCP connection success alone is reported as `tcp_connected`. These are observed
traffic results, not continuous HTTPS probes or a guarantee for every target.
A failure can concern a particular destination. A later successful request
updates the state. Existing QX traffic rules still decide which node to use.

A relay is published only while its DNS answer remains valid and its local
listener is reachable. Last upstream errors are reported separately, and make
the overall health partial. They do not delete imported nodes or change local
listener credentials. If no node is publishable, the subscription returns 503
so clients can retain their previous subscription.

DoH 临时传输失败最多尝试三次，总预算 10 秒；每次连接及 TLS 握手仍限制为
3 秒。HTTP 错误和格式错误的 DNS 响应不会盲目重试，也不会输出配置秘密。

中继在 TTL 的 70–85% 时提前刷新，按监听端口错开时间。冷缓存的并发请求共享
一次解析；后台刷新不阻塞有效缓存。失败后从 2 秒退避到最多 30 秒，原始 TTL
到期后不再使用该地址，始终不回退系统 DNS 或系统路由。持续的 DoH 故障仍需
修复端点或线路，重试不能保证外部网络可用。

状态分别呈现 DNS 和最近一次实际上游连接结果；收到协议解码后的响应才记录
verified，仅 TCP 连接成功记录 tcp_connected。状态属于已观察到的流量结果，
不是持续的 HTTPS 探测；某次错误也可能仅与目标站点有关。

DNS 仍有效且本地监听可达才发布中继。上游错误单独报告，整体健康状态标为
partial，但不删除节点、不更改监听鉴权；后续成功会更新状态。没有可发布节点
时订阅返回 503，让客户端保留上次订阅。QX 原有分流规则保持不变。

## Connection recovery / 建连恢复

Each relay allows at most 16 concurrent upstream dial operations (established
sessions retain the separate configured connection limit). Three consecutive
failed dial operations open a per-node circuit breaker. Further requests fail
quickly instead of queuing, spawning retry workers or repeatedly dialing.
The delay sequence is **2s, 5s, 15s, 45s, 60s, 2m, 5m, 15m, 30m, 45m, 1h**.
After each delay the next request performs one recovery attempt; concurrent
requests do not duplicate it. There are no idle/background TCP probes. Failure
of the final attempt sets `exhausted` and stops automatic connection recovery
until `dohsub restart`. A successful TCP dial resets the sequence; this does not
claim that the protocol or a particular destination is healthy.

A recovery attempt requests fresh DoH data while respecting resolver single-flight
and DNS failure backoff. Valid cached data may still be used if refresh fails;
expired data is never used. Multiple returned addresses share the 10-second TCP
budget, so one unreachable address cannot consume the entire budget. Existing
connections, node selection, credentials and traffic rules remain unchanged.
Routine TTL-based DNS maintenance remains independent of TCP recovery, including
when the TCP circuit is exhausted. Neither handshake errors nor destination I/O
errors trigger this node-wide TCP circuit breaker.

`relay_health` adds `recovery_state` (`ready`, `cooldown`, `probing`, `exhausted`),
`tcp_retry_at`, `consecutive_failures`, `suppressed`, `error_kind` and `dns_revision`.
`suppressed` is cumulative and published on recovery transitions and at most once per 30 seconds of suppressed traffic. `dns_revision`
changes when the DNS address set changes, without exposing addresses.
`error_kind` describes the most recent classified TCP failure (timeout, refused,
unreachable, address_unavailable, resource_exhausted or other).

Validated health snapshots are saved to `~/.dohsub/relay-diagnostics.jsonl`
(or `$DOHSUB_HOME/relay-diagnostics.jsonl`), mode 600, with Unix timestamps,
daemon PID and local relay port. Recovery transitions, DNS address changes,
DNS retry changes, first failure and helper exits are recorded; ordinary traffic
is summarized at most once per 30 seconds per relay. Rotation retains the current
file and one previous `.1` file, at most 1 MiB each. No credentials, raw helper
errors, upstream addresses or private DoH URLs are logged. Service lifecycle
messages in `service.log` also include timestamps and explicit stop requests.

每个中继最多同时进行 16 次上游建连，已建立会话仍使用独立的连接容量配置。
连续 3 次建连失败后进入该节点的冷却期，新请求快速失败，不排队、不创建重试
任务。间隔依次为 **2 秒、5 秒、15 秒、45 秒、60 秒、2 分钟、5 分钟、15 分钟、
30 分钟、45 分钟、1 小时**。每次到期后由下一条请求触发唯一一次恢复尝试；
最后一次仍失败则进入 exhausted，停止自动建连恢复，直到手动执行
`dohsub restart`。建连成功重置序列，但不等同于协议解密或目标网站访问成功。

恢复尝试会请求刷新 DoH，仍遵守解析合并及 DNS 失败退避。多地址共享 10 秒
TCP 预算，避免首个地址耗尽全部时间。不会切换节点、改变既有连接或分流规则。
常规 TTL DNS 维护独立运行，exhausted 只停止自动建连恢复；协议握手和目标网站
读写失败不会触发整个节点的 TCP 熔断。

健康接口增加恢复状态、下次建连时间、连续失败数、被抑制次数、错误分类和
DNS 地址集版本。被抑制次数在恢复状态变化时汇总发布，持续拒绝请求时最多每 30 秒更新一次。诊断日志使用时间戳、
进程号和本地监听端口关联，按上述规则记录并轮转，最多保留 2 MiB，不包含密钥、
上游地址或私有 DoH URL。重启后的历史记录仍可用于追查故障。

## AutoIP availability markers / AutoIP 可用性标记

AutoIP publication still requires a usable DNS address. A separate serial worker
checks supported native SS/SS2022 nodes every five minutes through the configured
physical interface, using the published IP and node credentials. It verifies TLS
certificates and an HTTP 2xx/3xx response from example.com or www.cloudflare.com.
Both targets must fail before the subscription tag gains ` · unreachable`.
Any successful target removes the marker. Checks take at most 16 seconds per node;
requests to the subscription endpoint never run probes. No system DNS, system proxy,
or alternate proxy node is used. Probes generate small HTTPS requests to these sites.

The original stored tag remains unchanged. Default and YAML subscriptions use the
same marker. Clients must refresh the subscription before name-based filtering
reflects changes; the marker does not disconnect existing sessions or change rules.
`/health` exposes `autoip_health` (reachable, unreachable, unknown). Results older
than 15 minutes, changed IP/credentials, missing physical interface and unsupported
obfuscation are unknown, not evidence of failure. Results are held in memory and
start unknown after restart. A probe result is a recent sample, not a guarantee
that every website is reachable. DNS-unavailable nodes remain omitted as before.

AutoIP 按已发布 IP 和原鉴权每 5 分钟串行实测一次，绑定配置中的物理接口；
通过节点访问上述两个 HTTPS 目标，均失败才加 ` · unreachable`，任一成功
则恢复原名。单节点最多约 16 秒，订阅请求不等待探测。原始配置名称不变，默认
订阅和 YAML 一致。QX 刷新订阅后可按该固定字符串过滤。未检测、结果超过
15 分钟、IP/鉴权变化、缺少物理接口或不支持的混淆配置均为 unknown，不标失败。
检测不使用系统代理、系统 DNS 或其他节点；会向上述网站产生少量 HTTPS 请求。

Unavailable name example: `Relay-US1 · AutoIP · unreachable`. The marker follows AutoIP; healthy node names retain their existing format.
不可用名称示例：`Relay-US1 · AutoIP · unreachable`；恢复后沿用原有正常名称格式。
