package main
import (
 "syscall"
 "golang.org/x/sys/unix"
)
func boundControl(index int,name string) func(string,string,syscall.RawConn)error {
 return func(network,address string,raw syscall.RawConn)error{
  var optionError error
  if err:=raw.Control(func(fd uintptr){optionError=unix.SetsockoptString(int(fd),unix.SOL_SOCKET,unix.SO_BINDTODEVICE,name)});err!=nil{return err}
  return optionError
 }
}
