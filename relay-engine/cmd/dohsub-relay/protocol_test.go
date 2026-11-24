package main
import (
 "context"
 "encoding/base64"
 "io"
 "net"
 "testing"
 "time"
 S "github.com/sagernet/sing-shadowsocks/shadowaead_2022"
 M "github.com/sagernet/sing/common/metadata"
 N "github.com/sagernet/sing/common/network"
)
type echoHandler struct{}
func(h *echoHandler)NewConnection(ctx context.Context,c net.Conn,m M.Metadata)error{defer c.Close();_,e:=io.Copy(c,c);return e}
func(h *echoHandler)NewPacketConnection(ctx context.Context,c N.PacketConn,m M.Metadata)error{return nil}
func(h *echoHandler)NewError(ctx context.Context,e error){}
func TestStandardRelayUsesStandardWire(t *testing.T){
 for _,size:=range []int{16,32}{for _,multi:=range []bool{false,true}{
  key:=make([]byte,size);key[0]=7;user:=make([]byte,size);user[0]=9
  method:="2022-blake3-aes-128-gcm";if size==32{method="2022-blake3-aes-256-gcm"}
  var cfg config;cfg.Node.Protocol="ss2022";cfg.Node.Method=method;cfg.Node.Password=base64.StdEncoding.EncodeToString(key)
  if multi{cfg.Node.Password+=":"+base64.StdEncoding.EncodeToString(user)}
  client,_,ascii,err:=methodForNode(cfg);if err!=nil || ascii{t.Fatal("standard path enabled Papaya envelope")}
  serverConn,clientConn:=net.Pipe();defer serverConn.Close();defer clientConn.Close();serverConn.SetDeadline(time.Now().Add(5*time.Second));clientConn.SetDeadline(time.Now().Add(5*time.Second))
  if multi{
   service,e:=S.NewMultiService[string](method,key,60,&echoHandler{},time.Now);if e!=nil{t.Fatal(e)};service.UpdateUsers([]string{"test"},[][]byte{user})
   go service.NewConnection(context.Background(),serverConn,M.Metadata{})
  }else{
   service,e:=S.NewService(method,key,60,&echoHandler{},time.Now);if e!=nil{t.Fatal(e)}
   go service.NewConnection(context.Background(),serverConn,M.Metadata{})
  }
  conn,e:=client.DialConn(clientConn,M.ParseSocksaddr("example.com:443"));if e!=nil{t.Fatal(e)}
  if _,e=conn.Write([]byte("probe"));e!=nil{t.Fatal(e)};b:=make([]byte,5);if _,e=io.ReadFull(conn,b);e!=nil || string(b)!="probe"{t.Fatal("standard response did not interoperate",e)};conn.Close()
 }}
}
func TestPapaya(t *testing.T){
 for _,name:=range []string{"papaya"}{var cfg config;cfg.Node.Protocol=name;cfg.Node.Auth.Server=base64.StdEncoding.EncodeToString(make([]byte,16));cfg.Node.Auth.User=cfg.Node.Auth.Server
  _,key,ascii,e:=methodForNode(cfg);if e!=nil || !ascii || len(key)!=16{t.Fatal("Papaya compatibility failed")}
 }
}
