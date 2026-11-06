// Optional compatibility check against an unpacked official static dashboard.
// U60_PANEL_ASSETS=/path/to/dashboard node zte-agent/ui/tests/proxy-panel.cjs
// Synthetic native API only; does not contact a router or capture screenshots.
const {chromium}=require('playwright'),assert=require('node:assert/strict'),http=require('node:http'),fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto');
(async()=>{
 const assets=process.env.U60_PANEL_ASSETS;if(!assets)throw new Error('Set U60_PANEL_ASSETS to an unpacked official dashboard');
 const prefix='/proxy-panel/mihomo/',api=prefix+'api/',token='a'.repeat(64),calls=[],streams=new Set(),errors=[];
 const replies={version:{meta:true,version:'v1.19.25'},configs:{mode:'rule','mixed-port':7890,'log-level':'info',ipv6:true,'allow-lan':false,tun:{enable:false},profile:{'store-selected':true}},proxies:{proxies:{DIRECT:{name:'DIRECT',type:'Direct',history:[],udp:true},GLOBAL:{name:'GLOBAL',type:'Selector',all:['DIRECT'],now:'DIRECT',history:[]}}},'providers/proxies':{providers:{}},'providers/rules':{providers:{}},rules:{rules:[]},connections:{connections:[],uploadTotal:0,downloadTotal:0},traffic:{up:0,down:0},memory:{inuse:12345,oslimit:0}};
 const server=http.createServer((req,res)=>{
  const url=new URL(req.url,'http://localhost');
  if(url.pathname.startsWith(api)){
   const name=url.pathname.slice(api.length);calls.push(name);
   assert.equal(req.headers.authorization,'Bearer '+token);res.setHeader('Content-Type','application/json');
   if(replies[name])return res.end(JSON.stringify(replies[name]));
   res.writeHead(404);return res.end('{"message":"Not provided by fixture"}');
  }
  if(!url.pathname.startsWith(prefix)){res.writeHead(404);return res.end();}
  const name=url.pathname.slice(prefix.length)||'index.html',file=path.resolve(assets,name);
  if(!file.startsWith(path.resolve(assets)+path.sep)||!fs.existsSync(file)||!fs.statSync(file).isFile()){res.writeHead(404);return res.end();}
  const mime={'.html':'text/html','.js':'text/javascript','.css':'text/css','.json':'application/json','.webmanifest':'application/json','.svg':'image/svg+xml','.png':'image/png','.ico':'image/x-icon','.woff2':'font/woff2','.woff':'font/woff','.ttf':'font/ttf'};
  res.setHeader('Content-Type',mime[path.extname(file)]||'application/octet-stream');
  res.setHeader('Content-Security-Policy',"default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; connect-src 'self' https: ws: wss:; img-src 'self' data: https:; font-src 'self' data:; worker-src 'self' blob:; base-uri 'self'; frame-ancestors 'none'");
  res.end(fs.readFileSync(file));
 });
 server.on('upgrade',(req,socket)=>{
  const url=new URL(req.url,'http://localhost');assert.equal(url.searchParams.get('token'),token);assert.ok(url.pathname.startsWith(api));calls.push('ws:'+url.pathname.slice(api.length));
  const accept=crypto.createHash('sha1').update(req.headers['sec-websocket-key']+'258EAFA5-E914-47DA-95CA-C5AB0DC85B11').digest('base64');
  socket.write(`HTTP/1.1 101 Switching Protocols\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Accept: ${accept}\r\n\r\n`);
  const data=Buffer.from(JSON.stringify(replies[url.pathname.slice(api.length)]||{type:'info',payload:'fixture log'}));assert.ok(data.length<126);
  const send=()=>socket.write(Buffer.concat([Buffer.from([0x81,data.length]),data]));send();const tick=setInterval(send,400);streams.add(socket);socket.on('error',()=>{});socket.on('close',()=>{clearInterval(tick);streams.delete(socket);});
 });
 await new Promise(r=>server.listen(0,'127.0.0.1',r));const origin=`http://127.0.0.1:${server.address().port}`;
 const browser=await chromium.launch({headless:true,channel:'chrome'}),page=await browser.newPage();page.on('pageerror',e=>errors.push(e.message));
 await page.route('**/*',route=>new URL(route.request().url()).origin===origin?route.continue():route.abort());
 try{
  const query=new URLSearchParams({hostname:new URL(origin).host+api.slice(0,-1),secret:token,http:'true'});
  await page.goto(origin+prefix+'#/setup?'+query);await page.waitForURL('**/#/overview',{timeout:30000});
  await page.waitForFunction(()=>localStorage.getItem('selectedEndpoint'));
  const endpoints=await page.evaluate(()=>JSON.parse(localStorage.getItem('endpointList')));assert.equal(endpoints[0].url,origin+api.slice(0,-1));assert.equal(endpoints[0].secret,token);
  await page.waitForTimeout(1800);assert.ok(calls.includes('version'));assert.ok(calls.includes('ws:traffic'));assert.ok(calls.includes('ws:memory'));assert.deepEqual(errors,[]);
  console.log('official dashboard: prefixed assets, automatic connection, scoped native API and live WebSocket streams passed');
 }finally{await browser.close();for(const stream of streams)stream.destroy();await new Promise(r=>server.close(r));}
})().catch(e=>{console.error(e);process.exitCode=1;});
