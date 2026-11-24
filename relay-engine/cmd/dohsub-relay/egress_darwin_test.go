package main
import (
 "net"
 "os"
 "syscall"
 "testing"
 "golang.org/x/sys/unix"
)
type rawSocket struct{fd int}
func(r rawSocket)Control(fn func(uintptr))error{fn(uintptr(r.fd));return nil}
func(r rawSocket)Read(fn func(uintptr)bool)error{return syscall.EINVAL}
func(r rawSocket)Write(fn func(uintptr)bool)error{return syscall.EINVAL}
func TestPhysicalSocketOption(t *testing.T){
 name:=os.Getenv("DOHSUB_TEST_INTERFACE");if name==""{t.Skip("explicit physical interface needed")}
 iface,err:=net.InterfaceByName(name);if err!=nil{t.Fatal(err)}
 fd,err:=unix.Socket(unix.AF_INET,unix.SOCK_STREAM,0);if err!=nil{t.Fatal(err)};defer unix.Close(fd)
 if err=boundControl(iface.Index,name)("tcp4","",rawSocket{fd});err!=nil{t.Fatal(err)}
 actual,err:=unix.GetsockoptInt(fd,unix.IPPROTO_IP,unix.IP_BOUND_IF)
 if err!=nil || actual!=iface.Index{t.Fatal("IP_BOUND_IF was not set")}
}
