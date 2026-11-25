# Service supervision / 服务监督

`dohsub run` runs the actual HTTP, DNS and relay service in the foreground. It
uses the saved configuration and binds the exact saved port. It never falls back
to another port during recovery. Configure DoH and import nodes before enabling
supervision. The foreground process can start while the network is unavailable;
DNS retries run in the background.

`dohsub run` 在前台运行实际订阅、DNS 和中继服务，使用已保存的配置与端口。
恢复时端口被占用会启动失败并由管理器重试，不会改变订阅地址。先配置 DoH
并导入节点，再启用监督。网络暂时不可用时仍能启动，DNS 在后台重试。

## macOS launchd

Create `~/Library/LaunchAgents/local.dohsub.subscription.plist`. Replace
`YOUR_HOME` with the absolute home directory and use the installed binary path:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>Label</key><string>local.dohsub.subscription</string>
  <key>ProgramArguments</key><array>
    <string>/opt/homebrew/bin/dohsub</string><string>run</string>
  </array>
  <key>EnvironmentVariables</key><dict>
    <key>HOME</key><string>YOUR_HOME</string>
    <key>DOHSUB_HOME</key><string>YOUR_HOME/.dohsub</string>
    <key>PATH</key><string>/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin</string>
  </dict>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><dict><key>SuccessfulExit</key><false/></dict>
  <key>ThrottleInterval</key><integer>5</integer>
  <key>StandardOutPath</key><string>YOUR_HOME/.dohsub/service.log</string>
  <key>StandardErrorPath</key><string>YOUR_HOME/.dohsub/service.log</string>
</dict></plist>
```

Stop any previously detached daemon before bootstrapping the job. Create the
private `~/.dohsub/launchd.json` file (mode 600) to route CLI `start/restart` back
to the registered job:

```json
{"label":"local.dohsub.subscription"}
```

```sh
dohsub stop
launchctl bootstrap "gui/$(id -u)" ~/Library/LaunchAgents/local.dohsub.subscription.plist
dohsub status
```

If replacing an already loaded job, `launchctl bootout` its label before
bootstrapping the replacement. Keep the plist and state files owned by your user.
Create log files with mode 600 before loading the job. Do not use a one-shot
wrapper calling `dohsub start`: that command detaches and exits, so launchd
cannot supervise the later daemon exit. Do not enable `AbandonProcessGroup`;
launchd must also clean up relay children after an unexpected exit.

启动前停止旧的脱离式守护进程；替换已加载的任务时先 bootout，再 bootstrap。
`launchd.json` 权限设为 600，日志也预先创建并设为 600。此文件存在但任务未加载时，
`start/restart` 会报错，避免悄悄启动一个不受监督的进程。不要使用仅调用
`dohsub start` 的一次性包装脚本，也不要启用 `AbandonProcessGroup`。

An unexpected exit or startup failure is retried by launchd. `dohsub stop`
requests a normal exit, so the service stays stopped until `dohsub start` or the
next login. `dohsub restart` validates DoH before stopping and starts through
launchd again. Startup and normal shutdown log the PID without credentials.

异常退出或启动失败会自动重试；显式 `stop` 正常退出并保持停止，直到 `start`
或下次登录。`restart` 先验证 DoH，再停止并通过 launchd 启动。日志记录启动和
正常停止的 PID，不记录节点密码或订阅令牌。

## Linux

Use `dohsub run` as a systemd user unit's `ExecStart`, with
`Restart=on-failure` and `RestartSec=5`. Set `DOHSUB_HOME` and an appropriate
private working directory. Use `systemctl --user start/stop/restart` for lifecycle
management; `launchd.json` is macOS-specific. Never wrap the detached `start`
command in a supervisor.
