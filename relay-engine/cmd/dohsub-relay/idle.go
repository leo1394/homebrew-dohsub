package main
import (
 "io"
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
func relayTraffic(client,remote net.Conn,timeout time.Duration){
 session:=&idleSession{left:client,right:remote,timeout:timeout};session.touch()
 left,right:=&idleConn{Conn:client,session:session},&idleConn{Conn:remote,session:session}
 done:=make(chan struct{},2)
 copyStream:=func(dst,src net.Conn){io.Copy(dst,src);client.Close();remote.Close();done<-struct{}{}}
 go copyStream(right,left);go copyStream(left,right);<-done;<-done
}
