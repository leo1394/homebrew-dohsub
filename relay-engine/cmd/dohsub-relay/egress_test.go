package main
import (
 "os"
 "errors"
 "sync"
 "sync/atomic"
 "path/filepath"
 "testing"
 "time"
)
func resetCache(){upstreamCache=newDNSCache(lookupUpstream)}
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

func TestRefreshDoesNotBlockValidTraffic(t *testing.T){
 now:=time.Now();started,release:=make(chan struct{}),make(chan struct{})
 cache:=newDNSCache(func(config)([]string,time.Duration,error){close(started);<-release;return nil,0,errors.New("temporary TLS timeout")})
 cache.now=func()time.Time{return now};cache.ips=[]string{"8.8.8.8"};cache.expires=now.Add(time.Minute)
 done:=make(chan struct{});go func(){cache.refresh(config{});close(done)}();<-started
 returned:=make(chan error,1);go func(){_,err:=cache.resolve(config{});returned<-err}()
 select{case err:=<-returned:if err!=nil{t.Fatal(err)};case <-time.After(time.Second):close(release);t.Fatal("valid cache waited for failing refresh")}
 close(release);<-done
 if _,err:=cache.resolve(config{});err!=nil{t.Fatal("refresh failure invalidated unexpired DNS")}
 now=now.Add(2*time.Minute)
 cache.Lock();cache.nextAttempt=now.Add(time.Minute);cache.Unlock()
 if _,err:=cache.resolve(config{});err==nil{t.Fatal("expired DNS was served during outage")}
}
func TestColdDNSIsSingleFlightAndFailuresBackOff(t *testing.T){
 var calls atomic.Int32;started,release:=make(chan struct{}),make(chan struct{})
 cache:=newDNSCache(func(config)([]string,time.Duration,error){calls.Add(1);close(started);<-release;return []string{"8.8.8.8"},time.Minute,nil})
 var workers sync.WaitGroup
 for i:=0;i<12;i++{workers.Add(1);go func(){defer workers.Done();if _,err:=cache.resolve(config{});err!=nil{t.Error(err)}}()}
 <-started;close(release);workers.Wait();if calls.Load()!=1{t.Fatal("cold requests amplified resolver calls")}
 var failed atomic.Int32
 cache=newDNSCache(func(config)([]string,time.Duration,error){failed.Add(1);return nil,0,errors.New("TLS timeout")})
 for i:=0;i<10;i++{if _,err:=cache.resolve(config{});err==nil{t.Fatal("failure accepted")}}
 if failed.Load()!=1{t.Fatal("failure backoff was bypassed")}
}
func TestBackgroundRefreshBeforeTTL(t *testing.T){
 var calls atomic.Int32;refreshed:=make(chan struct{},1)
 cache:=newDNSCache(func(config)([]string,time.Duration,error){if calls.Add(1)==2{refreshed<-struct{}{}};return []string{"8.8.8.8"},time.Second,nil})
 stop:=make(chan struct{});done:=make(chan struct{});go func(){cache.run(config{},stop);close(done)}();defer func(){close(stop);<-done}()
 select{case <-refreshed:case <-time.After(2*time.Second):t.Fatal("DNS was not refreshed before expiration")}
 cache.Lock();fresh:=time.Now().Before(cache.expires);cache.Unlock();if !fresh{t.Fatal("prefetch did not retain valid TTL")}
}

func TestScheduledRefreshReusesForegroundAnswer(t *testing.T){
 calls:=0
 cache:=newDNSCache(func(config)([]string,time.Duration,error){calls++;return []string{"8.8.8.8"},time.Minute,nil})
 if _,err:=cache.resolve(config{});err!=nil{t.Fatal(err)}
 if _,err:=cache.refresh(config{});err!=nil{t.Fatal(err)}
 if calls!=1{t.Fatal("scheduled refresh repeated a just-completed foreground lookup")}
}
