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
