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
var upstreamCache struct {sync.Mutex; ips []string; expires time.Time}
func validPublicV4(value string) bool {
 ip:=net.ParseIP(value);if ip==nil || ip.To4()==nil{return false}
 b:=ip.To4()
 return !ip.IsPrivate() && !ip.IsLoopback() && !ip.IsLinkLocalUnicast() && !ip.IsMulticast() && b[0]!=0 && b[0]<224 && !(b[0]==100 && b[1]>=64 && b[1]<=127) && !(b[0]==198 && (b[1]==18 || b[1]==19)) && !(b[0]==192 && b[1]==0 && (b[2]==2 || b[2]==0)) && !(b[0]==198 && b[1]==51 && b[2]==100) && !(b[0]==203 && b[1]==0 && b[2]==113)
}
func resolveUpstream(cfg config)([]string,error){
 upstreamCache.Lock();defer upstreamCache.Unlock()
 if time.Now().Before(upstreamCache.expires){return append([]string{},upstreamCache.ips...),nil}
 ctx,cancel:=context.WithTimeout(context.Background(),12*time.Second);defer cancel()
 data,err:=exec.CommandContext(ctx,cfg.Resolver,"__relay-resolve","--host",cfg.Node.Host,"--interface",cfg.Interface).Output()
 if err!=nil{return nil,errors.New("independent DoH failed; no system DNS fallback")}
 var reply struct {IPs []string `json:"ips"`;TTL int `json:"ttl"`}
 if len(data)>65536 || json.Unmarshal(data,&reply)!=nil || len(reply.IPs)==0 || reply.TTL<5 || reply.TTL>3600 {return nil,errors.New("invalid independent DNS result")}
 for _,ip:=range reply.IPs {if !validPublicV4(ip){return nil,errors.New("non-public upstream refused")}}
 upstreamCache.ips=reply.IPs;upstreamCache.expires=time.Now().Add(time.Duration(reply.TTL)*time.Second)
 return append([]string{},reply.IPs...),nil
}
func dialUpstream(cfg config)(net.Conn,error){
 iface,err:=net.InterfaceByName(cfg.Interface)
 if err!=nil || iface.Flags&net.FlagUp==0 || iface.Flags&net.FlagLoopback!=0 || strings.HasPrefix(cfg.Interface,"utun") || strings.HasPrefix(cfg.Interface,"tun"){return nil,errors.New("physical egress unavailable")}
 ips,err:=resolveUpstream(cfg);if err!=nil{return nil,err}
 dialer:=net.Dialer{Timeout:10*time.Second,Control:boundControl(iface.Index,cfg.Interface)}
 ctx,cancel:=context.WithTimeout(context.Background(),10*time.Second);defer cancel()
 for _,ip:=range ips {
  conn,e:=dialer.DialContext(ctx,"tcp4",net.JoinHostPort(ip,strconv.Itoa(cfg.Node.Port)));if e==nil{return conn,nil}
 }
 return nil,errors.New("physical upstream connection failed; no route fallback")
}
