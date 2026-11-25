package main
import (
 "encoding/json"
 "net"
 "os"
 "sync"
)
var outputLock sync.Mutex
func emit(value any){outputLock.Lock();defer outputLock.Unlock();json.NewEncoder(os.Stdout).Encode(value)}
func emitUpstream(state string){emit(struct{Event string `json:"event"`;State string `json:"upstream"`}{"upstream",state})}
type observedConn struct{net.Conn;verified bool}
func(c *observedConn)Read(p []byte)(int,error){
 n,err:=c.Conn.Read(p)
 if !c.verified{if n>0{c.verified=true;emitUpstream("verified")}else if err!=nil{emitUpstream("io_failed")}}
 return n,err
}
