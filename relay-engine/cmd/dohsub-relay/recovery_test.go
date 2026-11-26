package main
import (
 "errors"
 "encoding/base64"
 "context"
 "syscall"
 "net"
 "sync"
 "sync/atomic"
 "testing"
 "time"
)
func TestRecoveryBackoffAndSingleProbe(t *testing.T){
 now:=time.Now();gate:=newRecoveryGate();gate.now=func()time.Time{return now};gate.report=func(any){}
 calls,refreshes:=0,0
 fail:=func()(net.Conn,error){calls++;return nil,errors.New("offline")}
 refresh:=func(){refreshes++}
 for i:=0;i<3;i++{gate.dial(fail,refresh)}
 for i:=0;i<1000;i++{gate.dial(fail,refresh)}
 if calls!=3 || refreshes!=0{t.Fatal("requests bypassed failure cooldown",calls,refreshes)}
 for _,delay:=range []time.Duration{2,5,15,45,60,120,300}{
  if got:=gate.retryAt.Sub(now);got!=delay*time.Second{t.Fatal("unexpected backoff",got)}
  now=gate.retryAt;gate.dial(fail,refresh)
 }
 now=gate.retryAt
 entered,release:=make(chan struct{}),make(chan struct{});done:=make(chan struct{})
 go func(){gate.dial(func()(net.Conn,error){close(entered);<-release;return nil,nil},refresh);close(done)}();<-entered
 for i:=0;i<1000;i++{gate.dial(fail,refresh)}
 if calls!=10 || refreshes!=8{t.Fatal("half-open probe was duplicated",calls,refreshes)}
 close(release);<-done
 if gate.failures!=0 || !gate.retryAt.IsZero(){t.Fatal("successful probe did not restore traffic")}
 gate.dial(fail,refresh);if calls!=11{t.Fatal("recovery did not reopen")}
}
func TestRecoveryBoundsConcurrentDialsAndIgnoresOldResults(t *testing.T){
 gate:=newRecoveryGate();gate.report=func(any){}
 var calls atomic.Int32
 entered:=make(chan struct{},16);release:=make(chan struct{});var workers sync.WaitGroup
 for i:=0;i<16;i++{workers.Add(1);go func(){defer workers.Done();gate.dial(func()(net.Conn,error){calls.Add(1);entered<-struct{}{};<-release;return nil,errors.New("offline")},func(){})}()}
 for i:=0;i<16;i++{<-entered}
 for i:=0;i<1000;i++{gate.dial(func()(net.Conn,error){calls.Add(1);return nil,nil},func(){})}
 if calls.Load()!=16{t.Fatal("unbounded concurrent TCP attempts")}
 close(release);workers.Wait()
 if gate.failures!=3 || gate.pending!=0{t.Fatal("stale in-flight result changed recovery epoch",gate.failures,gate.pending)}
}
func TestRecoveryRefreshRespectsDNSBackoff(t *testing.T){
 now:=time.Now();calls:=0
 cache:=newDNSCache(func(config)([]string,time.Duration,error){calls++;return nil,0,errors.New("offline")});cache.now=func()time.Time{return now}
 cache.ips=[]string{"8.8.8.8"};cache.expires=now.Add(time.Minute);cache.refreshAt=now.Add(45*time.Second)
 cache.recover(config{});cache.recover(config{})
 if calls!=1{t.Fatal("forced refresh bypassed DNS backoff",calls)}
 if _,err:=cache.resolve(config{});err!=nil{t.Fatal("recovery discarded valid DNS")}
 now=now.Add(2*time.Minute)
 if _,err:=cache.resolve(config{});err==nil{t.Fatal("recovery reused expired DNS")}
}

func TestRecoveryStopsAfterLastScheduledAttempt(t *testing.T){
 now:=time.Now();gate:=newRecoveryGate();gate.now=func()time.Time{return now};gate.report=func(any){}
 calls:=0;attempt:=func()(net.Conn,error){calls++;return nil,errors.New("offline")}
 for i:=0;i<3;i++{gate.dial(attempt,func(){})}
 for _,seconds:=range []int{2,5,15,45,60,120,300,900,1800,2700,3600}{
  if gate.retryAt.Sub(now)!=time.Duration(seconds)*time.Second{t.Fatal("incorrect retry schedule",gate.retryAt.Sub(now))}
  now=gate.retryAt;gate.dial(attempt,func(){})
 }
 if !gate.exhausted || !gate.retryAt.IsZero() || calls!=14{t.Fatal("final failure did not stop recovery",calls)}
 now=now.Add(24*time.Hour)
 for i:=0;i<1000;i++{gate.dial(attempt,func(){t.Fatal("exhausted gate refreshed DNS")})}
 if calls!=14{t.Fatal("automatic retry resumed after exhaustion")}
}
func TestSOCKSRequestsShareUpstreamRecovery(t *testing.T){
 old:=upstreamRecovery;defer func(){upstreamRecovery=old}()
 gate:=newRecoveryGate();gate.report=func(any){};now:=time.Now();gate.now=func()time.Time{return now};upstreamRecovery=gate
 var cfg config;cfg.Interface="dohsub-no-such-interface";cfg.Username="u";cfg.Password="p";cfg.Node.Protocol="ss2022";cfg.Node.Method="2022-blake3-aes-128-gcm";cfg.Node.Password=base64.StdEncoding.EncodeToString(make([]byte,16))
 for i:=0;i<100;i++{
  client,server:=net.Pipe();done:=make(chan struct{});go func(){handle(server,cfg);close(done)}()
  client.SetDeadline(time.Now().Add(time.Second))
  for _,step:=range []struct{request []byte;length int}{{[]byte{5,1,2},2},{[]byte{1,1,'u',1,'p'},2},{[]byte{5,1,0,1,8,8,8,8,1,187},10}}{
   if _,err:=client.Write(step.request);err!=nil{t.Fatal(err)}
   if _,err:=readBytes(client,step.length);err!=nil{t.Fatal(err)}
  }
  client.Close();<-done
 }
 if gate.failures!=3 || gate.suppressed!=97{t.Fatal("SOCKS traffic bypassed shared recovery",gate.failures,gate.suppressed)}
}
func TestMultipleAddressesShareDialBudget(t *testing.T){
 ctx,cancel:=context.WithTimeout(context.Background(),time.Second);defer cancel();deadline,_:=ctx.Deadline();calls:=0
 left,right:=net.Pipe();defer left.Close();defer right.Close()
 conn,err:=dialAddresses(ctx,[]string{"8.8.8.8","1.1.1.1"},443,func(attempt context.Context,network,address string)(net.Conn,error){
  calls++;end,_:=attempt.Deadline()
  if calls==1{if deadline.Sub(end)<400*time.Millisecond{t.Fatal("first address consumed full budget")};return nil,context.DeadlineExceeded}
  if network!="tcp4" || address!="1.1.1.1:443"{t.Fatal("wrong alternate address")};return left,nil
 })
 if err!=nil || conn!=left || calls!=2{t.Fatal("alternate address not attempted")}
 if dialErrorKind(&net.OpError{Err:syscall.ECONNREFUSED})!="refused" || dialErrorKind(context.DeadlineExceeded)!="timeout"{t.Fatal("TCP failure classification lost")}
}
