package main
import (
 "io"
 "net"
 "sync"
 "testing"
 "time"
)
func TestCapacityCountsRejectsAndRecovers(t *testing.T){
 for _,limit:=range []int{16,256}{
  l,err:=net.Listen("tcp","127.0.0.1:0");if err!=nil{t.Fatal(err)}
  var lock sync.Mutex;var state capacityStatus
  report:=func(v any){lock.Lock();state=v.(capacityStatus);lock.Unlock()}
  snapshot:=func()capacityStatus{lock.Lock();defer lock.Unlock();return state}
  done:=make(chan struct{});go func(){serveClients(l,limit,func(c net.Conn){c.Write([]byte{1});io.Copy(io.Discard,c)},report);close(done)}()
  clients:=[]net.Conn{}
  connect:=func()net.Conn{c,e:=net.DialTimeout("tcp",l.Addr().String(),time.Second);if e!=nil{t.Fatal(e)};c.SetDeadline(time.Now().Add(3*time.Second));clients=append(clients,c);return c}
  for i:=0;i<limit;i++{var b [1]byte;if _,e:=io.ReadFull(connect(),b[:]);e!=nil{t.Fatalf("connection %d rejected: %v",i+1,e)}}
  extra:=connect();var b [1]byte;if _,e:=extra.Read(b[:]);e==nil{t.Fatal("overload accepted")}
  if s:=snapshot();s.Active!=limit || s.Peak!=limit || s.Rejected!=1{t.Fatalf("bad counters: %+v",s)}
  clients[0].Close();deadline:=time.Now().Add(time.Second)
  for snapshot().Active==limit && time.Now().Before(deadline){time.Sleep(time.Millisecond)}
  if _,e:=io.ReadFull(connect(),b[:]);e!=nil{t.Fatal("capacity did not recover",e)}
  for _,c:=range clients{c.Close()};l.Close();<-done
  deadline=time.Now().Add(time.Second);for snapshot().Active!=0 && time.Now().Before(deadline){time.Sleep(time.Millisecond)}
  if snapshot().Active!=0{t.Fatal("active count leaked")}
 }
}
