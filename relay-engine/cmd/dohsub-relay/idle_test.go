package main
import (
 "io"
 "errors"
 "net"
 "testing"
 "time"
)
func TestRelayActivityRenewsBothDirections(t *testing.T){
 for _,reverse:=range []bool{false,true}{
  app,left:=net.Pipe();right,server:=net.Pipe();done:=make(chan struct{})
  go func(){relayTraffic(left,right,300*time.Millisecond);close(done)}()
  sender,receiver:=app,server;if reverse{sender,receiver=server,app}
  sender.SetDeadline(time.Now().Add(4*time.Second));receiver.SetDeadline(time.Now().Add(4*time.Second))
  received:=make(chan error,1);go func(){_,err:=io.CopyN(io.Discard,receiver,12);received<-err}()
  for i:=0;i<12;i++{if _,err:=sender.Write([]byte{1});err!=nil{t.Fatal("active stream expired",err)};time.Sleep(60*time.Millisecond)}
  if err:=<-received;err!=nil{t.Fatal(err)}
  select{case <-done:t.Fatal("active stream closed");default:}
  select{case <-done:case <-time.After(time.Second):t.Fatal("idle stream did not expire")}
  app.Close();server.Close()
 }
}
func TestRelayPeerCloseReleasesBothCopies(t *testing.T){
 for _,closeClient:=range []bool{false,true}{
  app,left:=net.Pipe();right,server:=net.Pipe();done:=make(chan struct{})
  go func(){relayTraffic(left,right,time.Minute);close(done)}()
  if closeClient{app.Close()}else{server.Close()}
  select{case <-done:case <-time.After(time.Second):t.Fatal("peer close left a copy goroutine blocked")}
  app.Close();server.Close()
 }
}

func TestRelayReportsIdleAndEOF(t *testing.T){
 for _,idle:=range []bool{false,true}{
  app,left:=net.Pipe();right,server:=net.Pipe();done:=make(chan string,1)
  go func(){done<-relayTraffic(left,right,30*time.Millisecond)}()
  expected:="idle_timeout";if !idle{expected="eof";app.Close()}
  select{case reason:=<-done:if reason!=expected{t.Fatalf("got %s, want %s",reason,expected)};case <-time.After(time.Second):t.Fatal("relay stuck")}
  app.Close();server.Close()
 }
}

type brokenReadConn struct{net.Conn}
func(c *brokenReadConn)Read(p []byte)(int,error){return 0,errors.New("synthetic transfer failure")}
func TestRelayReportsTransferFailure(t *testing.T){
 app,left:=net.Pipe();right,server:=net.Pipe();defer app.Close();defer server.Close()
 if reason:=relayTraffic(left,&brokenReadConn{right},time.Second);reason!="download_io_failed"{t.Fatal(reason)}
}
