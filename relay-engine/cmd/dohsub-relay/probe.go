package main
import (
 "bufio"
 "context"
 "crypto/tls"
 "encoding/json"
 "fmt"
 "io"
 "net"
 "net/http"
 "os"
 "strings"
 "time"
 A "github.com/sagernet/sing-shadowsocks/shadowaead"
 S "github.com/sagernet/sing-shadowsocks/shadowaead_2022"
 SS "github.com/sagernet/sing-shadowsocks"
 M "github.com/sagernet/sing/common/metadata"
)
// Separate bounded process: no listener, system proxy, or system DNS lookup.
func probeMain(){
 var input struct{IP,Interface,Method,Password string;Port int}
 result:="unknown"
 defer func(){json.NewEncoder(os.Stdout).Encode(map[string]string{"result":result})}()
 if json.NewDecoder(io.LimitReader(os.Stdin,65536)).Decode(&input)!=nil || !validPublicV4(input.IP){return}
 iface,err:=net.InterfaceByName(input.Interface)
 if err!=nil || iface.Flags&net.FlagUp==0 || iface.Flags&net.FlagLoopback!=0 || strings.HasPrefix(input.Interface,"utun") || strings.HasPrefix(input.Interface,"tun"){return}
 var method SS.Method
 if strings.HasPrefix(input.Method,"2022-"){method,err=S.NewWithPassword(input.Method,input.Password,time.Now)}else{method,err=A.New(input.Method,nil,input.Password)}
 if err!=nil{return}
 // Two unrelated destinations reduce false failures caused by one website.
 for _,host:=range []string{"example.com","www.cloudflare.com"}{
  if probeHTTPS(input.IP,input.Port,iface.Index,input.Interface,method,host){result="reachable";return}
 }
 result="unreachable"
}
func probeHTTPS(ip string,port,index int,iface string,method SS.Method,host string)bool{
 ctx,cancel:=context.WithTimeout(context.Background(),8*time.Second);defer cancel()
 dialer:=net.Dialer{Timeout:4*time.Second,Control:boundControl(index,iface)}
 conn,err:=dialer.DialContext(ctx,"tcp4",net.JoinHostPort(ip,fmt.Sprint(port)));if err!=nil{return false};defer conn.Close()
 deadline,_:=ctx.Deadline();conn.SetDeadline(deadline)
 remote,err:=method.DialConn(conn,M.ParseSocksaddr(host+":443"));if err!=nil{return false}
 secure:=tls.Client(remote,&tls.Config{ServerName:host,MinVersion:tls.VersionTLS12})
 if secure.HandshakeContext(ctx)!=nil{return false}
 if _,err=fmt.Fprintf(secure,"GET / HTTP/1.1\r\nHost: %s\r\nConnection: close\r\n\r\n",host);err!=nil{return false}
 response,err:=http.ReadResponse(bufio.NewReader(secure),nil);if err!=nil{return false};response.Body.Close()
 return response.StatusCode>=200 && response.StatusCode<400
}
