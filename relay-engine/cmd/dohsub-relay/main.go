package main
import (
 "crypto/rand"
 "errors"
 SS "github.com/sagernet/sing-shadowsocks"
 "encoding/base64"
 "encoding/binary"
 "encoding/json"
 "fmt"
 "io"
 "net"
 "os"
 "strconv"
 "time"
 "syscall"
 S "github.com/sagernet/sing-shadowsocks/shadowaead_2022"
 M "github.com/sagernet/sing/common/metadata"
 "lukechampine.com/blake3"
)
type asciiConn struct { net.Conn; server []byte; first bool }
func (c *asciiConn) Write(p []byte) (int,error) {
 if c.first { return c.Conn.Write(p) }
 c.first=true
 nonce:=make([]byte,6);if _,err:=rand.Read(nonce);err!=nil {return 0,err}
 header:=make([]byte,9);header[0]=1
 binary.BigEndian.PutUint32(header[1:5],uint32(len(p)))
 binary.BigEndian.PutUint32(header[5:9],0)
 mask:=make([]byte,32);material:=append(append([]byte{},c.server...),nonce...)
 blake3.DeriveKey(mask,"papaya v1 envelope header mask",material)
 for i:=range header {header[i]^=mask[i]}
 wire:=append(append(nonce,header...),p...)
 encoded:=[]byte(base64.NewEncoding("0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ-_").WithPadding(base64.NoPadding).EncodeToString(wire))
 for len(encoded)>0 {n,err:=c.Conn.Write(encoded);if err!=nil{return 0,err};if n==0{return 0,io.ErrShortWrite};encoded=encoded[n:]}
 return len(p),nil
}
type config struct {
 Resolver string `json:"resolver"`
 Interface string `json:"interface"`
 ListenPort int `json:"listen_port"`
 Username string `json:"username"`
 Password string `json:"password"`
 Node struct { Host string `json:"host"`; Port int `json:"port"`; Protocol string `json:"protocol"`; Method string `json:"method"`; Password string `json:"password"`; Params map[string]any `json:"params"`; Auth struct {Server string `json:"server_key"`; User string `json:"user_key"`} `json:"auth"` } `json:"node"`
}
func readBytes(c net.Conn,n int)([]byte,error){ b:=make([]byte,n);_,e:=io.ReadFull(c,b);return b,e }
func handle(c net.Conn,cfg config) {
 defer c.Close();c.SetDeadline(time.Now().Add(15*time.Second))
 h,e:=readBytes(c,2);if e!=nil || h[0]!=5{return};methods,e:=readBytes(c,int(h[1]));if e!=nil{return};ok:=false;for _,m:=range methods{if m==2{ok=true}}
 if !ok{c.Write([]byte{5,255});return};c.Write([]byte{5,2})
 h,e=readBytes(c,2);if e!=nil || h[0]!=1{return};u,e:=readBytes(c,int(h[1]));if e!=nil{return};h,e=readBytes(c,1);if e!=nil{return};p,e:=readBytes(c,int(h[0]));if e!=nil{return}
 if string(u)!=cfg.Username || string(p)!=cfg.Password{c.Write([]byte{1,1});return};c.Write([]byte{1,0})
 h,e=readBytes(c,4);if e!=nil || h[0]!=5 || h[1]!=1 || h[2]!=0{c.Write([]byte{5,7,0,1,0,0,0,0,0,0});return}
 host:="";switch h[3]{case 1:b,e:=readBytes(c,4);if e!=nil{return};host=net.IP(b).String();case 4:b,e:=readBytes(c,16);if e!=nil{return};host=net.IP(b).String();case 3:b,e:=readBytes(c,1);if e!=nil || b[0]==0{return};b,e=readBytes(c,int(b[0]));if e!=nil{return};host=string(b);default:return}
 b,e:=readBytes(c,2);if e!=nil{return};port:=binary.BigEndian.Uint16(b);if port==0{return}
 m,server,papaya,e:=methodForNode(cfg);if e!=nil{return}
 upstream,e:=dialUpstream(cfg);if e!=nil{c.Write([]byte{5,4,0,1,0,0,0,0,0,0});return};defer upstream.Close()
 upstream.SetDeadline(time.Now().Add(5*time.Minute))
 var wrapped net.Conn=upstream
 if papaya {wrapped=&asciiConn{Conn:upstream,server:server}}
 remote,e:=m.DialConn(wrapped,M.ParseSocksaddr(net.JoinHostPort(host,strconv.Itoa(int(port)))));if e!=nil{emitUpstream("handshake_failed");c.Write([]byte{5,1,0,1,0,0,0,0,0,0});return}
 c.Write([]byte{5,0,0,1,0,0,0,0,0,0});c.SetDeadline(time.Now().Add(5*time.Minute))
 remote=&observedConn{Conn:remote}
 done:=make(chan struct{},1);go func(){io.Copy(remote,c);done<-struct{}{}}();io.Copy(c,remote);c.Close();upstream.Close();<-done
}
func methodForNode(cfg config)(SS.Method,[]byte,bool,error){
 papaya:=cfg.Node.Protocol=="papaya"
 method,password:=cfg.Node.Method,cfg.Node.Password
 var server []byte
 if papaya {
  var err error;server,err=base64.StdEncoding.DecodeString(cfg.Node.Auth.Server);if err!=nil{return nil,nil,false,errors.New("invalid key")};user,err:=base64.StdEncoding.DecodeString(cfg.Node.Auth.User);if err!=nil{return nil,nil,false,errors.New("invalid key")}
  method="2022-blake3-aes-128-gcm";if len(user)==32{method="2022-blake3-aes-256-gcm"};if (len(user)!=16 && len(user)!=32)||len(server)!=len(user){return nil,nil,false,errors.New("invalid key lengths")}
  password=cfg.Node.Auth.Server+":"+cfg.Node.Auth.User
 }
 if !papaya && (cfg.Node.Protocol!="ss2022" || (method!="2022-blake3-aes-128-gcm" && method!="2022-blake3-aes-256-gcm")){return nil,nil,false,errors.New("unsupported method")}
 m,e:=S.NewWithPassword(method,password,time.Now);if e!=nil{return nil,nil,false,errors.New("invalid keys")}
 if papaya {m.(*S.Method).SetTCPContexts("papaya v1 identity subkey","papaya v1 session subkey")}
 return m,server,papaya,nil
}
func main(){
 parent:=os.Getppid();go func(){for{time.Sleep(time.Second);if syscall.Getppid()!=parent{os.Exit(0)}}}()
 var cfg config
 if json.NewDecoder(io.LimitReader(os.Stdin,1048576)).Decode(&cfg)!=nil || (cfg.Node.Protocol!="papaya" && cfg.Node.Protocol!="ss2022") || cfg.ListenPort<1 || cfg.ListenPort>65535 || cfg.Resolver=="" || cfg.Interface=="" || cfg.Username=="" || cfg.Password=="" || len(cfg.Node.Params)!=0 {os.Exit(2)}
 if _,_,_,err:=methodForNode(cfg);err!=nil{os.Exit(2)}
 l,e:=net.Listen("tcp",net.JoinHostPort("127.0.0.1",strconv.Itoa(cfg.ListenPort)));if e!=nil{os.Exit(3)};defer l.Close();fmt.Println("READY")
 upstreamCache.report=func(value dnsStatus){emit(value)}
 stop:=make(chan struct{});defer close(stop);go upstreamCache.run(cfg,stop)
 slots:=make(chan struct{},32)
 for{c,e:=l.Accept();if e!=nil{return};select{case slots<-struct{}{}:go func(){defer func(){<-slots}();handle(c,cfg)}();default:c.Close()}}
}
