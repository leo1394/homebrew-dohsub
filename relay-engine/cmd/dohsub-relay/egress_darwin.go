package main
import (
 "syscall"
 "golang.org/x/sys/unix"
)
func boundControl(index int,name string) func(string,string,syscall.RawConn)error {
 return func(network,address string,raw syscall.RawConn)error{
  var optionError error
  if err:=raw.Control(func(fd uintptr){optionError=unix.SetsockoptInt(int(fd),unix.IPPROTO_IP,unix.IP_BOUND_IF,index)});err!=nil{return err}
  return optionError
 }
}
