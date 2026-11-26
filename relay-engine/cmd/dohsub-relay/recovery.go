package main
import (
 "errors"
 "net"
 "sync"
 "time"
)
// One gate per upstream/helper. Existing established connections are unaffected.
type recoveryGate struct {
 sync.Mutex
 pending,failures int
 epoch,suppressed uint64
 retryAt time.Time
 lastReport time.Time
 probing bool
 exhausted bool
 now func()time.Time
 report func(any)
}
type recoveryStatus struct {Event string `json:"event"`;State string `json:"recovery_state"`;RetryAt int64 `json:"tcp_retry_at"`;Failures int `json:"consecutive_failures"`;Suppressed uint64 `json:"suppressed"`}
func newRecoveryGate()*recoveryGate{return &recoveryGate{now:time.Now,report:emit}}
var upstreamRecovery=newRecoveryGate()
var recoveryDelays=[]time.Duration{2*time.Second,5*time.Second,15*time.Second,45*time.Second,time.Minute,2*time.Minute,5*time.Minute,15*time.Minute,30*time.Minute,45*time.Minute,time.Hour}
func(g *recoveryGate)publishLocked(state string){
 g.lastReport=g.now()
 retry:=int64(0);if !g.retryAt.IsZero(){retry=g.retryAt.Unix()}
 g.report(recoveryStatus{"recovery",state,retry,g.failures,g.suppressed})
}
func(g *recoveryGate)dial(attempt func()(net.Conn,error),refresh func())(net.Conn,error){
 g.Lock()
 recovering:=!g.retryAt.IsZero()
 if g.exhausted || g.pending>=16 || g.probing || (recovering && (g.now().Before(g.retryAt) || g.pending>0)) {
  g.suppressed++
  if g.now().Sub(g.lastReport)>=30*time.Second {
   state:="ready";if g.exhausted{state="exhausted"}else if g.probing{state="probing"}else if recovering{state="cooldown"};g.publishLocked(state)
  }
  g.Unlock();return nil,errors.New("upstream recovery cooldown or dial limit; retry later")
 }
 epoch:=g.epoch;g.pending++
 if recovering{g.probing=true;g.publishLocked("probing")}
 g.Unlock()
 // The single half-open request refreshes DNS; no background TCP probe or
 // per-request sleep/goroutine is created while the upstream is unavailable.
 if recovering{refresh()}
 conn,err:=attempt()
 g.Lock();defer g.Unlock();g.pending--
 if epoch!=g.epoch{return conn,err}
 g.probing=false
 if err==nil {
  g.failures=0;g.retryAt=time.Time{}
  if recovering{g.epoch++;g.publishLocked("ready")}
 }else{
  g.failures++
  if g.failures>=3 {
   g.epoch++
   if g.failures-3>=len(recoveryDelays) {g.exhausted=true;g.retryAt=time.Time{};g.publishLocked("exhausted")}else{g.retryAt=g.now().Add(recoveryDelays[g.failures-3]);g.publishLocked("cooldown")}
  }
 }
 return conn,err
}
