This independent GPL-3.0-or-later helper contains code from SagerNet/sing-shadowsocks
commit 854fa8eed597b323ae28772357d84f1dec231620. See LICENSE.
Local protocol changes add per-instance TCP derivation contexts; standard defaults
remain unchanged. Papaya mode uses papaya contexts and the default ASCII first-request
envelope. Standard SS2022 mode retains standard contexts and no envelope.
TCP CONNECT only; unknown protocol parameters are rejected.
The Rust subscription application communicates with this separate executable over stdin.
Cargo build invokes Go for the selected target and embeds the separate helper
executable. The daemon extracts it under its private state directory and verifies
its bytes before execution. Stable installations need no adjacent helper file.
