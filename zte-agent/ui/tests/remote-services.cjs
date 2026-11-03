// Private-network and proxy controls use synthetic data; no device is contacted.
const {chromium}=require('playwright'),assert=require('node:assert/strict'),http=require('node:http'),fs=require('node:fs'),path=require('node:path');
(async()=>{
 const ok=data=>({ok:true,data}),writes=[],streams=new Set(),errors=[];
 const proxy={installed:true,configured:true,enabled:false,running:false,capture:'local',capturing:false,mode:'rule',version:'1.0.0',local_endpoint:'127.0.0.1:7890'};
 const ts={installed:true,enabled:true,enrolled:false,daemon_running:true,backend_state:'NeedsLogin',gateway_mode:true,management_port:9091,accept_routes:true,accept_routes_effective:true,hostname:'example-router',advertise_routes:[],hold_wakelock:true,tailscale_ssh:false,client_version:'1.104.4',health:[]};
 const groups=[{name:'Primary group',selected:'Example A',choices:['Example A','Example B'],selectable:true,kind:'Selector'},{name:'<img src=x onerror=alert(1)>',selected:'Example A',choices:['Example A'],selectable:false,kind:'URLTest'}];
 const core={device:ok({hostname:'Example router',uptime_secs:100}),wan:ok({up:true}),wan6:ok({up:false}),signal:ok({network_type:'5G'}),modem:ok({operate_mode:'ONLINE'}),battery:ok({capacity:80}),wifi:ok({}),clients:ok({hosts:{}}),speed:ok({rx_speed:0,tx_speed:0})};
 let holdStatus=false,releaseStatus=null;
 const server=http.createServer((req,res)=>{
  if(req.url.startsWith('/api/dashboard/events')){res.writeHead(200,{'Content-Type':'text/event-stream'});streams.add(res);res.on('close',()=>streams.delete(res));res.write(`event: snapshot\ndata: ${JSON.stringify({readings:core})}\n\n`);return;}
  if(req.url.startsWith('/api/')){
   const send=data=>{res.setHeader('Content-Type','application/json');res.end(JSON.stringify(ok(data)));};
   if(req.method==='GET'){
    if(req.url==='/api/proxy/status'){if(holdStatus){releaseStatus=()=>send(proxy);return;}return send(proxy);}
    if(req.url==='/api/tailscale/status')return send(ts);
    if(req.url==='/api/proxy/groups')return send(groups);
    return send({});
   }
   let raw='';req.on('data',b=>raw+=b);req.on('end',()=>{
    const body=JSON.parse(raw||'{}');writes.push({url:req.url,body});
    if(req.url==='/api/proxy/settings')Object.assign(proxy,body);
    if(req.url==='/api/proxy/install')proxy.version=body.version;
    if(req.url==='/api/proxy/enable')Object.assign(proxy,{enabled:true,running:true,capturing:proxy.capture==='hotspot'});
    if(req.url==='/api/proxy/disable')Object.assign(proxy,{enabled:false,running:false,capturing:false});
    if(req.url==='/api/proxy/select')groups[0].selected=body.choice;
    if(req.url==='/api/tailscale/config')Object.assign(ts,body,{accept_routes_effective:body.accept_routes});
    if(req.url==='/api/tailscale/login-url'){ts.login_url='https://login.tailscale.com/a/example-only';return send({login_url:ts.login_url});}
    if(req.url==='/api/tailscale/setup')Object.assign(ts,{enrolled:true,backend_state:'Running',tailscale_ips:['100.101.102.103'],dns_name:'example-router.example.ts.net'});
    if(req.url==='/api/tailscale/disable')Object.assign(ts,{enabled:false,backend_state:'Stopped'});
    if(req.url==='/api/tailscale/update')ts.updating=true;
    send(req.url.startsWith('/api/proxy/')?proxy:ts);
   });return;
  }
  const name=req.url==='/'?'index.html':/^\/console\/a\/[\w.-]+$/.test(req.url)?req.url.split('/').pop():null;
  const file=name&&path.join(__dirname,'..',name);if(!file||!fs.existsSync(file)){res.writeHead(404);return res.end();}res.setHeader('Content-Type',name.endsWith('.js')?'text/javascript':name.endsWith('.css')?'text/css':'text/html');res.end(fs.readFileSync(file));
 });
 await new Promise(r=>server.listen(0,'127.0.0.1',r));const browser=await chromium.launch({headless:true,channel:'chrome'}),page=await browser.newPage();page.on('pageerror',e=>errors.push(e.message));
 const done=()=>page.waitForFunction(()=>!document.documentElement.dataset.saving);
 const confirm=async label=>{await page.locator('.confirm-dialog').getByRole('button',{name:label,exact:true}).click();await page.waitForFunction(()=>!document.querySelector('.confirm-dialog').open);await done();};
 try{
  await page.goto(`http://127.0.0.1:${server.address().port}/#network/proxy`);await page.waitForFunction(()=>OpenU60Console.connected&&!document.querySelector('#proxy-policy button[type=submit]').disabled);
  await page.locator('#proxy-mode').selectOption('global');await page.locator('#proxy-capture').selectOption('hotspot');
  await page.getByRole('button',{name:'Save traffic policy',exact:true}).click();await page.locator('.confirm-dialog').getByRole('button',{name:'Cancel',exact:true}).click();await page.waitForFunction(()=>!document.querySelector('.confirm-dialog').open);assert.equal(writes.length,0);
  await page.getByRole('button',{name:'Save traffic policy',exact:true}).click();await confirm('Apply change');await page.waitForFunction(()=>document.querySelector('#proxy-capture').value==='hotspot');assert.deepEqual(writes[0],{url:'/api/proxy/settings',body:{mode:'global',capture:'hotspot'}});
  const sample='proxies: [{name: Example, type: socks5, server: example.com, port: 1080}]';
  await page.locator('#proxy-file').setInputFiles({name:'example.yaml',mimeType:'text/yaml',buffer:Buffer.from(sample)});await page.waitForFunction(()=>document.querySelector('#proxy-config').value.includes('example.com'));
  await page.getByRole('button',{name:'Import configuration',exact:true}).click();await confirm('Apply change');assert.equal(writes.at(-1).body.config,sample);assert.equal(await page.locator('#proxy-config').inputValue(),'');
  await page.locator('#proxy-source-url').fill('https://example.com/config?token=demo-private');await page.getByRole('button',{name:'Fetch configuration',exact:true}).click();assert.ok(!(await page.locator('.confirm-dialog').textContent()).includes('demo-private'));await confirm('Apply change');assert.equal(await page.locator('#proxy-source-url').inputValue(),'');
  await page.locator('#proxy-binary-url').fill('https://downloads.example.com/engine.gz');await page.locator('#proxy-version').fill('1.1.0');await page.locator('#proxy-sha256').fill('a'.repeat(64));await page.getByRole('button',{name:'Install engine',exact:true}).click();await confirm('Install engine');assert.equal(writes.at(-1).url,'/api/proxy/install');
  await page.getByRole('button',{name:'Start service',exact:true}).click();await confirm('Start service');await page.getByLabel('Primary group',{exact:true}).waitFor();
  assert.equal(await page.locator('.service-groups img').count(),0);assert.equal(await page.getByRole('button',{name:'Save traffic policy',exact:true}).isDisabled(),true);
  await page.getByLabel('Primary group',{exact:true}).selectOption('Example B');await page.locator('.service-group').first().getByRole('button',{name:'Use node'}).click();await confirm('Apply change');assert.deepEqual(writes.at(-1),{url:'/api/proxy/select',body:{group:'Primary group',choice:'Example B'}});
  groups[1].name='Automatic group';await page.locator('#proxy-service').getByRole('button',{name:'Refresh status',exact:true}).click();await page.getByLabel('Automatic group',{exact:true}).waitFor();
  const images='/tmp/codex/u60-services-images';fs.mkdirSync(images,{recursive:true});
  for(const theme of ['light','dark'])for(const width of [390,820,1440]){
   await page.evaluate(t=>{document.documentElement.dataset.theme=t;},theme);await page.setViewportSize({width,height:1000});await page.waitForTimeout(80);assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);await page.evaluate(()=>scrollTo(0,0));if(width===1440)await page.screenshot({path:`${images}/proxy-${theme}.png`,fullPage:true});
  }
  await page.evaluate(()=>OpenU60Navigation.go('network/tailscale'));await page.waitForFunction(()=>!document.querySelector('#tailscale-policy button[type=submit]').disabled);
  assert.match(await page.locator('#tailscale-service').textContent(),/Remote web port.*9091/s);
  await page.getByRole('button',{name:'Authorize this router',exact:true}).click();await confirm('Get authorization link');assert.equal(await page.getByRole('link',{name:'Open authorization page'}).getAttribute('href'),'https://login.tailscale.com/a/example-only');
  await page.locator('#tailscale-routes').fill('192.0.2.0/24, 198.51.100.0/24');await page.locator('#tailscale-accept').selectOption('false');await page.getByRole('button',{name:'Save network settings',exact:true}).click();await confirm('Apply change');assert.deepEqual(writes.at(-1).body.advertise_routes,['192.0.2.0/24','198.51.100.0/24']);assert.equal(writes.at(-1).body.accept_routes,false);
  await page.locator('#tailscale-auth-key').fill('demo-private-auth');await page.getByRole('button',{name:'Enroll router',exact:true}).click();assert.ok(!(await page.locator('.confirm-dialog').textContent()).includes('demo-private-auth'));await confirm('Enroll router');assert.equal(await page.locator('#tailscale-auth-key').inputValue(),'');await page.waitForFunction(()=>document.querySelector('#tailscale-service .service-summary').textContent==='Connected');
  assert.equal(await page.getByRole('link',{name:'Open authorization page'}).count(),0);
  await page.evaluate(()=>scrollTo(0,0));await page.screenshot({path:`${images}/tailscale-desktop.png`,fullPage:true});await page.setViewportSize({width:390,height:844});assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);await page.screenshot({path:`${images}/tailscale-mobile.png`});
  assert.equal(await page.locator('#tailscale-version').inputValue(),'1.104.4');await page.locator('#tailscale-sha256').fill('b'.repeat(64));await page.getByRole('button',{name:'Install version',exact:true}).click();await confirm('Install version');assert.equal(writes.at(-1).body.version,'1.104.4');
  // A pending confirmation is invalidated by leaving the page.
  ts.updating=false;await page.locator('#tailscale-service').getByRole('button',{name:'Refresh status',exact:true}).click();await page.waitForFunction(()=>!document.querySelector('#tailscale-policy button[type=submit]').disabled);
  const before=writes.length;await page.getByRole('button',{name:'Stop Tailscale',exact:true}).click();await page.evaluate(()=>OpenU60Navigation.go('network/proxy'));await confirm('Stop Tailscale');assert.equal(writes.length,before);
  // A late authenticated reply cannot repopulate private state after lock.
  holdStatus=true;await page.locator('#proxy-service').getByRole('button',{name:'Refresh status',exact:true}).click();await page.waitForTimeout(100);
  await page.locator('#proxy-config').fill('private draft');await page.locator('#proxy-source-url').fill('private subscription');await page.evaluate(()=>window.dispatchEvent(new Event('dashboard:locked')));releaseStatus?.();await page.waitForTimeout(100);
  assert.equal(await page.locator('#proxy-config').inputValue(),'');assert.equal(await page.locator('#proxy-source-url').inputValue(),'');assert.equal(await page.locator('#proxy-service .service-summary').textContent(),'Waiting for status');assert.equal(writes.length,before);assert.deepEqual(errors,[]);
  console.log('remote services: policy, import, installation, node selection, Tailscale, stale-session protection and responsive layouts passed');
 }finally{for(const stream of streams)stream.end();await browser.close();await new Promise(r=>server.close(r));}
})().catch(e=>{console.error(e);process.exitCode=1;});
