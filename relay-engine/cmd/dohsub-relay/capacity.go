package main
import (
 "net"
 "sync"
)
type capacityStatus struct {Event string `json:"event"`;Active int `json:"active"`;Peak int `json:"peak"`;Limit int `json:"limit"`;Rejected uint64 `json:"rejected"`}
// Publish while locked so concurrent completions cannot overwrite newer counts.
func serveClients(l net.Listener,limit int,handle func(net.Conn),report func(any)){
 var lock sync.Mutex
 state:=capacityStatus{Event:"capacity",Limit:limit};report(state)
 for{
  c,err:=l.Accept();if err!=nil{return}
  lock.Lock()
  if state.Active>=limit{state.Rejected++;report(state);lock.Unlock();c.Close();continue}
  state.Active++;if state.Active>state.Peak{state.Peak=state.Active};report(state);lock.Unlock()
  go func(){defer func(){c.Close();lock.Lock();state.Active--;report(state);lock.Unlock()}();handle(c)}()
 }
}
