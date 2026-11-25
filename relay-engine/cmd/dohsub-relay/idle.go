package main
import (
 "io"
 "errors"
 "net"
 "sync"
 "time"
)
// Both directions share one idle window; a download also keeps the upload read alive.
type idleSession struct {sync.Mutex; left,right net.Conn; timeout time.Duration}
func(s *idleSession)touch(){s.Lock();defer s.Unlock();deadline:=time.Now().Add(s.timeout);s.left.SetDeadline(deadline);s.right.SetDeadline(deadline)}
type idleConn struct {net.Conn; session *idleSession}
func(c *idleConn)Read(p []byte)(int,error){n,err:=c.Conn.Read(p);if n>0{c.session.touch()};return n,err}
func(c *idleConn)Write(p []byte)(int,error){n,err:=c.Conn.Write(p);if n>0{c.session.touch()};return n,err}
func relayTraffic(client,remote net.Conn,timeout time.Duration)string{
 session:=&idleSession{left:client,right:remote,timeout:timeout};session.touch()
 left,right:=&idleConn{Conn:client,session:session},&idleConn{Conn:remote,session:session}
 done:=make(chan string,2);var once sync.Once
 copyStream:=func(dst,src net.Conn,direction string){
  _,err:=io.Copy(dst,src)
  once.Do(func(){reason:="eof";if err!=nil{var timeout net.Error;if errors.As(err,&timeout) && timeout.Timeout(){reason="idle_timeout"}else{reason=direction+"_io_failed"}};done<-reason;client.Close();remote.Close()})
  done<-"finished"
 }
 go copyStream(right,left,"upload");go copyStream(left,right,"download");reason:=<-done;<-done;<-done;return reason
}
