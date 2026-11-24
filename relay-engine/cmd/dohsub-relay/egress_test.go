package main
import (
 "os"
 "path/filepath"
 "testing"
 "time"
)
func resetCache(){upstreamCache.Lock();upstreamCache.ips=nil;upstreamCache.expires=time.Time{};upstreamCache.Unlock()}
func resolverFixture(t *testing.T,text string) string {
 t.Helper();p:=filepath.Join(t.TempDir(),"resolver");if err:=os.WriteFile(p,[]byte("#!/bin/sh\nprintf '%s' '"+text+"'\n"),0700);err!=nil{t.Fatal(err)};return p
}
func TestIndependentDNSCacheAndExpiry(t *testing.T){
 resetCache();defer resetCache()
 var cfg config;cfg.Resolver=resolverFixture(t,`{"ips":["8.8.8.8"],"ttl":5}`);cfg.Interface="en0";cfg.Node.Host="fixture.example"
 ips,err:=resolveUpstream(cfg);if err!=nil || len(ips)!=1{t.Fatal("independent resolver failed")}
 if err:=os.Remove(cfg.Resolver);err!=nil{t.Fatal(err)}
 if _,err:=resolveUpstream(cfg);err!=nil{t.Fatal("unexpired cache not used")}
 upstreamCache.expires=time.Now().Add(-time.Second)
 if _,err:=resolveUpstream(cfg);err==nil{t.Fatal("expired address or system DNS fallback was used")}
}
func TestRejectFakeAndPrivateDNS(t *testing.T){
 for _,ip:=range []string{"198.18.0.2","198.19.1.2","127.0.0.1","192.168.1.1","100.64.1.1","::1"}{
  resetCache();var cfg config;cfg.Resolver=resolverFixture(t,`{"ips":["`+ip+`"],"ttl":60}`)
  if _,err:=resolveUpstream(cfg);err==nil{t.Fatal("non-public DNS result accepted")}
 };resetCache()
}
func TestMissingInterfaceNeverFallsBack(t *testing.T){
 var cfg config;cfg.Interface="dohsub-no-such-interface"
 if _,err:=dialUpstream(cfg);err==nil{t.Fatal("missing physical interface accepted")}
}
