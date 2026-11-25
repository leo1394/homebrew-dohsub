package main
import (
 "context"
 "encoding/json"
 "errors"
 "net"
 "os/exec"
 "strconv"
 "strings"
 "sync"
 "time"
)
type dnsCache struct {
 sync.Mutex
 ips []string
 expires,refreshAt,nextAttempt time.Time
 refreshing chan struct{}
 failures int
 lookup func(config)([]string,time.Duration,error)
 now func()time.Time
 report func(dnsStatus)
}
type dnsStatus struct {Event string `json:"event"`;Expires int64 `json:"dns_expires"`;State string `json:"dns_state"`;RetryAt int64 `json:"retry_at"`}
func newDNSCache(lookup func(config)([]string,time.Duration,error))*dnsCache{return &dnsCache{lookup:lookup,now:time.Now}}
var upstreamCache=newDNSCache(lookupUpstream)
func validPublicV4(value string) bool {
 ip:=net.ParseIP(value);if ip==nil || ip.To4()==nil{return false}
 b:=ip.To4()
 return !ip.IsPrivate() && !ip.IsLoopback() && !ip.IsLinkLocalUnicast() && !ip.IsMulticast() && b[0]!=0 && b[0]<224 && !(b[0]==100 && b[1]>=64 && b[1]<=127) && !(b[0]==198 && (b[1]==18 || b[1]==19)) && !(b[0]==192 && b[1]==0 && (b[2]==2 || b[2]==0)) && !(b[0]==198 && b[1]==51 && b[2]==100) && !(b[0]==203 && b[1]==0 && b[2]==113)
}
func lookupUpstream(cfg config)([]string,time.Duration,error){
 ctx,cancel:=context.WithTimeout(context.Background(),12*time.Second);defer cancel()
 data,err:=exec.CommandContext(ctx,cfg.Resolver,"__relay-resolve","--host",cfg.Node.Host,"--interface",cfg.Interface).Output()
 if err!=nil{return nil,0,errors.New("independent DoH failed; no system DNS fallback")}
 var reply struct {IPs []string `json:"ips"`;TTL int `json:"ttl"`}
 if len(data)>65536 || json.Unmarshal(data,&reply)!=nil || len(reply.IPs)==0 || reply.TTL<5 || reply.TTL>3600 {return nil,0,errors.New("invalid independent DNS result")}
 for _,ip:=range reply.IPs {if !validPublicV4(ip){return nil,0,errors.New("non-public upstream refused")}}
 return reply.IPs,time.Duration(reply.TTL)*time.Second,nil
}
func(c *dnsCache)statusLocked()dnsStatus{
 state:="pending"
 if c.now().Before(c.expires){state="ready"}
 if c.refreshing!=nil{state="refreshing"}else if c.failures>0{state="retrying"}
 expires,retry:=int64(0),int64(0);if !c.expires.IsZero(){expires=c.expires.Unix()};if !c.nextAttempt.IsZero(){retry=c.nextAttempt.Unix()}
 return dnsStatus{Event:"dns",Expires:expires,State:state,RetryAt:retry}
}
func(c *dnsCache)publish(){if c.report!=nil{c.Lock();value:=c.statusLocked();c.Unlock();c.report(value)}}
func(c *dnsCache)cachedLocked()([]string,error){
 if len(c.ips)>0 && c.now().Before(c.expires){return append([]string{},c.ips...),nil}
 return nil,errors.New("independent DNS unavailable; no expired address or system DNS fallback")
}
func(c *dnsCache)resolve(cfg config)([]string,error){
 c.Lock();ips,err:=c.cachedLocked();c.Unlock()
 if err==nil{return ips,nil}
 return c.refresh(cfg)
}
func(c *dnsCache)refresh(cfg config)([]string,error){
 c.Lock()
 if done:=c.refreshing;done!=nil{c.Unlock();<-done;c.Lock();defer c.Unlock();return c.cachedLocked()}
 if c.now().Before(c.refreshAt) && c.now().Before(c.expires){defer c.Unlock();return c.cachedLocked()}
 if c.now().Before(c.nextAttempt){defer c.Unlock();return c.cachedLocked()}
 done:=make(chan struct{});c.refreshing=done;c.Unlock();c.publish()
 ips,ttl,err:=c.lookup(cfg)
 c.Lock();now:=c.now()
 if err==nil{
  c.ips=append([]string{},ips...);c.expires=now.Add(ttl)
  c.refreshAt=now.Add(ttl*time.Duration(70+cfg.ListenPort%16)/100)
  c.nextAttempt=time.Time{};c.failures=0
 }else{
  if c.failures<5{c.failures++};delay:=time.Duration(1<<c.failures)*time.Second;if delay>30*time.Second{delay=30*time.Second}
  c.nextAttempt=now.Add(delay);c.refreshAt=c.nextAttempt
 }
 c.refreshing=nil;close(done);result,resultErr:=c.cachedLocked();c.Unlock();c.publish()
 return result,resultErr
}
func(c *dnsCache)run(cfg config,stop <-chan struct{}){
 timer:=time.NewTimer(time.Duration(cfg.ListenPort%1000)*time.Millisecond);defer timer.Stop()
 for{
  select{case <-stop:return;case <-timer.C:}
  c.refresh(cfg)
  c.Lock();delay:=c.refreshAt.Sub(c.now());c.Unlock();if delay<100*time.Millisecond{delay=100*time.Millisecond};timer.Reset(delay)
 }
}
func resolveUpstream(cfg config)([]string,error){return upstreamCache.resolve(cfg)}
func dialUpstream(cfg config)(net.Conn,error){
 iface,err:=net.InterfaceByName(cfg.Interface)
 if err!=nil || iface.Flags&net.FlagUp==0 || iface.Flags&net.FlagLoopback!=0 || strings.HasPrefix(cfg.Interface,"utun") || strings.HasPrefix(cfg.Interface,"tun"){emitUpstream("interface_failed");return nil,errors.New("physical egress unavailable")}
 ips,err:=resolveUpstream(cfg);if err!=nil{emitUpstream("dns_failed");return nil,err}
 dialer:=net.Dialer{Timeout:10*time.Second,Control:boundControl(iface.Index,cfg.Interface)}
 ctx,cancel:=context.WithTimeout(context.Background(),10*time.Second);defer cancel()
 for _,ip:=range ips {
  conn,e:=dialer.DialContext(ctx,"tcp4",net.JoinHostPort(ip,strconv.Itoa(cfg.Node.Port)));if e==nil{emitUpstream("tcp_connected");return conn,nil}
 }
 emitUpstream("tcp_failed")
 return nil,errors.New("physical upstream connection failed; no route fallback")
}
