// Private-network and proxy controls use synthetic data; no device is contacted.
const {chromium}=require('playwright'),assert=require('node:assert/strict'),http=require('node:http'),fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto');
(async()=>{
 const uploads={};
 const software={mihomo:{phase:'idle',busy:false,available:null},tailscale:{phase:'idle',busy:false,available:null}};
 let releaseFailure=false,chunkFailure=false,loseAcceptance=false,loseOfficialAcceptance=false,holdRelease=false,releaseMetadata=null;
 const ok=data=>({ok:true,data}),writes=[],streams=new Set(),errors=[];
 const proxy={id:'mihomo',name:'mihomo',mixed_port:7890,ipv6:true,tcp_concurrent:false,installed:true,configured:false,config_revision:null,enabled:false,running:false,capture:'local',capturing:false,mode:'rule',version:'1.0.0',local_endpoint:'127.0.0.1:7890'};
 const ts={installed:true,enabled:true,enrolled:false,daemon_running:true,backend_state:'NeedsLogin',gateway_mode:true,management_port:9091,accept_routes:true,accept_routes_effective:true,hostname:'example-router',advertise_routes:[],hold_wakelock:true,tailscale_ssh:false,client_version:'1.104.4',health:[]};
 const groups=[{name:'Primary group',selected:'Example A',choices:['Example A','Example B'],selectable:true,kind:'Selector'},{name:'<img src=x onerror=alert(1)>',selected:'Example A',choices:['Example A'],selectable:false,kind:'URLTest'}];
 const core={device:ok({hostname:'Example router',uptime_secs:100}),wan:ok({up:true}),wan6:ok({up:false}),signal:ok({network_type:'5G'}),modem:ok({operate_mode:'ONLINE'}),battery:ok({capacity:80}),wifi:ok({}),clients:ok({hosts:{}}),speed:ok({rx_speed:0,tx_speed:0})};
 let holdStatus=false,releaseStatus=null,holdConfig=false,releaseConfig=null;
 let savedConfig='',ignoreSave=false,rejectConfig=false,reloads=0;
 const setConfig=text=>{savedConfig=text;proxy.configured=true;proxy.config_revision=crypto.createHash('sha256').update(text).digest('hex');};
 const other={id:'fixture-service',name:'Fixture service',running:true,installed:true,local_endpoint:'127.0.0.1:7899'};
 const server=http.createServer((req,res)=>{
  if(req.url.startsWith('/api/dashboard/events')){res.writeHead(200,{'Content-Type':'text/event-stream'});streams.add(res);res.on('close',()=>streams.delete(res));res.write(`event: snapshot\ndata: ${JSON.stringify({readings:core})}\n\n`);return;}
  if(req.url.startsWith('/api/')){
   const send=data=>{res.setHeader('Content-Type','application/json');res.end(JSON.stringify(ok(data)));};
   if(req.url.startsWith('/api/software/')){
    const [, , ,service,action]=req.url.split('/');
    const available={version:service==='mihomo'?'1.1.0':'1.104.5',name:service==='mihomo'?'mihomo-linux-arm64-v1.1.0.gz':'tailscale_1.104.5_arm64.tgz',sha256:'a'.repeat(64),architecture:'linux-arm64'};
    if(req.method==='GET'){
     if(action==='release'){
      if(releaseFailure){res.statusCode=503;return res.end(JSON.stringify({ok:false,error:'Official source unavailable in fixture'}));}
      const checked=()=>{software[service].available=available;send(software[service]);};
      if(holdRelease){releaseMetadata=checked;return;}checked();return;
     }
     return send(software[service]);
    }
    let raw='';req.on('data',b=>raw+=b);req.on('end',()=>{
     const body=JSON.parse(raw);writes.push({url:req.url,body});
     if(req.url.endsWith('/upload')){uploads[service]={bytes:Buffer.alloc(0),version:'1.2.3',active:true};Object.assign(software[service],{phase:'uploading',busy:true});return send({upload_id:'fixture-upload-'+service,chunk_size:262144});}
     if(req.url.endsWith('/upload/chunk')){
      if(chunkFailure){res.statusCode=400;return res.end(JSON.stringify({ok:false,error:'Upload interrupted in fixture'}));}
      const upload=uploads[service];assert.equal(body.offset,upload.bytes.length);upload.bytes=Buffer.concat([upload.bytes,Buffer.from(body.data,'base64')]);return send({bytes_received:upload.bytes.length});
     }
     if(req.url.endsWith('/upload/cancel')){
      if(!uploads[service]?.active){res.statusCode=400;return res.end(JSON.stringify({ok:false,error:'Upload identifier invalid'}));}
      uploads[service].active=false;Object.assign(software[service],{phase:'idle',busy:false});return send(software[service]);
     }
     if(req.url.endsWith('/upload/install')){
      if(!uploads[service]?.active){res.statusCode=400;return res.end(JSON.stringify({ok:false,error:'Upload identifier invalid'}));}
      uploads[service].active=false;Object.assign(software[service],{phase:'preparing',busy:true});
      setTimeout(()=>{proxy.version=uploads[service].version;Object.assign(software[service],{phase:'succeeded',busy:false});},400);
      if(loseAcceptance){res.writeHead(200,{'Content-Type':'application/json','Content-Length':2000});res.write('{"ok":true,"data":');setImmediate(()=>res.destroy());return;}return send(software[service]);
     }
     if(service==='mihomo')proxy.version=body.version;else ts.client_version=body.version;
     if(loseOfficialAcceptance){Object.assign(software[service],{phase:'installing',busy:true,available});setTimeout(()=>Object.assign(software[service],{phase:'succeeded',busy:false}),400);res.writeHead(200,{'Content-Type':'application/json','Content-Length':2000});res.write('{"ok":true,"data":');setImmediate(()=>res.destroy());return;}
     Object.assign(software[service],{phase:'succeeded',busy:false,available});send(software[service]);
    });return;
   }
   if(req.method==='GET'){
    if(req.url==='/api/proxy/services')return send({services:[proxy,other]});
    if(req.url==='/api/proxy/mihomo/config'){const read=()=>send({config:savedConfig,configured:proxy.configured,revision:proxy.config_revision});if(holdConfig){releaseConfig=read;return;}return read();}
    if(req.url==='/api/proxy/mihomo/status'){if(holdStatus){releaseStatus=()=>send(proxy);return;}return send(proxy);}
    if(req.url==='/api/tailscale/status')return send(ts);
    if(req.url==='/api/proxy/mihomo/groups')return send(groups);
    return send({});
   }
   let raw='';req.on('data',b=>raw+=b);req.on('end',()=>{
    const body=JSON.parse(raw||'{}');writes.push({url:req.url,body});
    if(['/api/proxy/mihomo/config','/api/proxy/mihomo/subscription','/api/proxy/mihomo/validate','/api/proxy/mihomo/reload'].includes(req.url)){
     if(body.revision!==proxy.config_revision){res.statusCode=409;return res.end(JSON.stringify({ok:false,error:'Configuration changed since it was read'}));}
     if(rejectConfig){res.statusCode=400;return res.end(JSON.stringify({ok:false,error:'Fixture configuration rejected'}));}
     if(req.url.endsWith('/validate'))return send({valid:true,revision:proxy.config_revision});
     if(req.url.endsWith('/reload'))reloads++;
     else if(!ignoreSave)setConfig(body.config||'proxies: [{name: Subscribed, type: socks5, server: example.com, port: 1080}]');
     return send(proxy);
    }
    if(req.url==='/api/proxy/mihomo/settings')Object.assign(proxy,body);
    if(req.url==='/api/proxy/mihomo/install')proxy.version=body.version;
    if(req.url==='/api/proxy/mihomo/enable')Object.assign(proxy,{enabled:true,running:true,capturing:proxy.capture==='hotspot'});
    if(req.url==='/api/proxy/mihomo/disable')Object.assign(proxy,{enabled:false,running:false,capturing:false});
    if(req.url==='/api/proxy/mihomo/select')groups[0].selected=body.choice;
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
  await page.getByRole('button',{name:'Save runtime settings',exact:true}).click();await page.locator('.confirm-dialog').getByRole('button',{name:'Cancel',exact:true}).click();await page.waitForFunction(()=>!document.querySelector('.confirm-dialog').open);assert.equal(writes.length,0);
  await page.getByRole('button',{name:'Save runtime settings',exact:true}).click();await confirm('Apply change');await page.waitForFunction(()=>document.querySelector('#proxy-capture').value==='hotspot');assert.deepEqual(writes[0],{url:'/api/proxy/mihomo/settings',body:{mode:'global',capture:'hotspot',mixed_port:7890,ipv6:true,tcp_concurrent:false}});
  assert.equal(await page.locator('#proxy-directory select').count(),0);
  assert.match(await page.locator('#proxy-directory').textContent(),/Fixture service/);
  assert.equal(await page.getByRole('button',{name:'Start mihomo',exact:true}).isDisabled(),true);
  await page.getByRole('tab',{name:'Configuration',exact:true}).click();
  await page.waitForFunction(()=>document.querySelector('#proxy-config-feedback').textContent.includes('No saved configuration'));
  assert.equal(await page.locator('#proxy-source-url').getAttribute('type'),'url');
  assert.equal(await page.locator('#proxy-import-file').isDisabled(),true);
  const sample='proxies: [{name: Example, type: socks5, server: example.com, port: 1080}]';
  await page.locator('#proxy-file').setInputFiles({name:'example.yaml',mimeType:'text/yaml',buffer:Buffer.from(sample)});await page.waitForFunction(()=>document.querySelector('#proxy-config').value.includes('example.com'));
  await page.getByRole('button',{name:'Validate configuration',exact:true}).click();await done();assert.equal(proxy.configured,false);assert.match(await page.locator('#proxy-config-feedback').textContent(),/No changes have been applied/);
  ignoreSave=true;await page.getByRole('button',{name:'Save configuration',exact:true}).click();await confirm('Apply configuration');assert.match(await page.locator('#proxy-config-feedback').textContent(),/could not be confirmed/);assert.equal(await page.locator('#proxy-config').inputValue(),sample);assert.equal(await page.getByRole('button',{name:'Start mihomo',exact:true}).isDisabled(),true);ignoreSave=false;
  await page.locator('#proxy-import-file').click();await confirm('Apply configuration');assert.equal(writes.at(-1).body.config,sample);assert.equal(await page.locator('#proxy-config').inputValue(),sample);assert.equal(await page.getByRole('button',{name:'Start mihomo',exact:true}).isDisabled(),false);
  await page.locator('#proxy-source-url').fill('https://example.com/config?token=demo-private');await page.getByRole('button',{name:'Import and apply subscription',exact:true}).click();assert.ok(!(await page.locator('.confirm-dialog').textContent()).includes('demo-private'));await confirm('Apply configuration');assert.equal(await page.locator('#proxy-source-url').inputValue(),'');assert.match(await page.locator('#proxy-config').inputValue(),/Subscribed/);
  await page.getByRole('tab',{name:'Installation',exact:true}).click();
  releaseFailure=true;await page.locator('#mihomo-check-update').click();await page.waitForFunction(()=>document.querySelector('#mihomo-installer').textContent.includes('Official source unavailable in fixture'));assert.equal(await page.locator('#mihomo-install-release').isDisabled(),true);releaseFailure=false;
  const beforeCheck=writes.length;await page.locator('#mihomo-check-update').click();await page.waitForFunction(()=>!document.querySelector('#mihomo-install-release').disabled);assert.equal(writes.length,beforeCheck);await page.locator('#mihomo-install-release').click();await confirm('Install / upgrade');assert.equal(writes.at(-1).url,'/api/software/mihomo/install');assert.equal(writes.at(-1).body.version,'1.1.0');
  await page.locator('#mihomo-installer .software-manual summary').click();
  const packageBytes=Buffer.alloc(400000,73);await page.locator('#mihomo-package-file').setInputFiles({name:'mihomo-linux-arm64-v1.2.3.gz',mimeType:'application/gzip',buffer:packageBytes});
  const beforeUpload=writes.length;await page.locator('#mihomo-upload-install').click();await page.locator('.confirm-dialog').getByRole('button',{name:'Cancel',exact:true}).click();await page.waitForFunction(()=>!document.querySelector('.confirm-dialog').open);assert.equal(writes.length,beforeUpload);
  chunkFailure=true;await page.locator('#mihomo-upload-install').click();await confirm('Upload and install');assert.match(await page.locator('#mihomo-installer').textContent(),/Upload interrupted in fixture/);assert.equal(proxy.version,'1.1.0');assert.equal(software.mihomo.busy,false);chunkFailure=false;
  const beforeRetry=writes.length;loseAcceptance=true;await page.locator('#mihomo-upload-install').click();await confirm('Upload and install');
  await page.waitForFunction(()=>document.querySelector('#mihomo-installer').textContent.includes('1.2.3'));
  assert.equal(uploads.mihomo.bytes.equals(packageBytes),true);assert.equal(writes.slice(beforeRetry).filter(w=>w.url.endsWith('/upload/chunk')).length,2);assert.equal(writes.slice(beforeRetry).filter(w=>w.url.endsWith('/upload/install')).length,1);loseAcceptance=false;
  assert.equal(await page.locator('#mihomo-package-file').inputValue(),'');
  await page.getByRole('button',{name:'Start mihomo',exact:true}).click();await confirm('Start mihomo');assert.equal(other.running,true);assert.ok(writes.filter(w=>w.url.startsWith('/api/proxy/')).every(w=>w.url.startsWith('/api/proxy/mihomo/')));
  await page.getByRole('tab',{name:'Configuration',exact:true}).click();
  const previousConfig=savedConfig,previousRevision=proxy.config_revision,writeStart=writes.length;
  await page.locator('#proxy-config').fill(sample+"\nrules: ['MATCH,DIRECT']");
  rejectConfig=true;await page.getByRole('button',{name:'Save and reload',exact:true}).click();await confirm('Apply configuration');assert.match(await page.locator('#proxy-config-feedback').textContent(),/Fixture configuration rejected/);assert.equal(savedConfig,previousConfig);assert.equal(proxy.running,true);rejectConfig=false;
  await page.getByRole('button',{name:'Save and reload',exact:true}).click();await confirm('Apply configuration');assert.equal(writes.at(-1).body.revision,previousRevision);assert.equal(proxy.running,true);assert.ok(writes.slice(writeStart).every(w=>w.url==='/api/proxy/mihomo/config'));
  const applied=savedConfig;await page.locator('#proxy-config').fill('private unsaved draft');await page.locator('#proxy-reload-config').click();await confirm('Reload');assert.equal(reloads,1);assert.equal(savedConfig,applied);assert.equal(await page.locator('#proxy-config').inputValue(),'private unsaved draft');assert.match(await page.locator('#proxy-config-feedback').textContent(),/still unsaved/);
  setConfig(sample+'\n# changed elsewhere');await page.getByRole('button',{name:'Save and reload',exact:true}).click();await confirm('Apply configuration');assert.match(await page.locator('#proxy-config-feedback').textContent(),/Configuration changed/);assert.equal(await page.locator('#proxy-config').inputValue(),'private unsaved draft');
  await page.getByRole('button',{name:'Load saved configuration',exact:true}).click();await confirm('Discard and read');await page.waitForFunction(()=>document.querySelector('#proxy-config').value.includes('changed elsewhere'));
  await page.getByRole('tab',{name:'Proxy groups',exact:true}).click();await page.getByLabel('Primary group',{exact:true}).waitFor();
  assert.equal(await page.locator('.service-groups img').count(),0);assert.equal(await page.locator('#proxy-policy button[type=submit]').isDisabled(),true);
  await page.getByLabel('Primary group',{exact:true}).selectOption('Example B');await page.locator('#proxy-service').getByRole('button',{name:'Refresh status',exact:true}).click();await page.waitForTimeout(150);assert.equal(await page.getByLabel('Primary group',{exact:true}).inputValue(),'Example B');await page.locator('.service-group').first().getByRole('button',{name:'Use node'}).click();await confirm('Apply change');assert.deepEqual(writes.at(-1),{url:'/api/proxy/mihomo/select',body:{group:'Primary group',choice:'Example B'}});
  groups[1].name='Automatic group';await page.locator('#proxy-service').getByRole('button',{name:'Refresh status',exact:true}).click();await page.getByLabel('Automatic group',{exact:true}).waitFor();
  await page.getByRole('tab',{name:'Configuration',exact:true}).click();
  assert.ok(await page.locator('#proxy-save-config').evaluate(button=>button.getBoundingClientRect().top<document.querySelector('#proxy-config').getBoundingClientRect().top));
  // No proxy-page screenshots: branding is deliberately excluded from published assets.
  const images='/tmp/codex/u60-services-images';fs.mkdirSync(images,{recursive:true});
  for(const theme of ['light','dark'])for(const width of [390,820,1440]){
   await page.evaluate(t=>{document.documentElement.dataset.theme=t;},theme);await page.setViewportSize({width,height:1000});await page.waitForTimeout(80);assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);await page.evaluate(()=>scrollTo(0,0));
  }
  await page.evaluate(()=>OpenU60Navigation.go('network/tailscale'));await page.waitForFunction(()=>!document.querySelector('#tailscale-policy button[type=submit]').disabled);
  assert.match(await page.locator('#tailscale-service').textContent(),/Remote web port.*9091/s);
  await page.getByRole('button',{name:'Authorize this router',exact:true}).click();await confirm('Get authorization link');assert.equal(await page.getByRole('link',{name:'Open authorization page'}).getAttribute('href'),'https://login.tailscale.com/a/example-only');
  await page.locator('#tailscale-routes').fill('192.0.2.0/24, 198.51.100.0/24');await page.locator('#tailscale-accept').selectOption('false');await page.getByRole('button',{name:'Save network settings',exact:true}).click();await confirm('Apply change');assert.deepEqual(writes.at(-1).body.advertise_routes,['192.0.2.0/24','198.51.100.0/24']);assert.equal(writes.at(-1).body.accept_routes,false);
  await page.locator('#tailscale-auth-key').fill('demo-private-auth');await page.getByRole('button',{name:'Enroll router',exact:true}).click();assert.ok(!(await page.locator('.confirm-dialog').textContent()).includes('demo-private-auth'));await confirm('Enroll router');assert.equal(await page.locator('#tailscale-auth-key').inputValue(),'');await page.waitForFunction(()=>document.querySelector('#tailscale-service .service-summary').textContent==='Connected');
  assert.equal(await page.getByRole('link',{name:'Open authorization page'}).count(),0);
  await page.evaluate(()=>scrollTo(0,0));await page.screenshot({path:`${images}/tailscale-desktop.png`,fullPage:true});await page.setViewportSize({width:390,height:844});assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);await page.screenshot({path:`${images}/tailscale-mobile.png`});
  await page.locator('#tailscale-check-update').click();await page.waitForFunction(()=>!document.querySelector('#tailscale-install-release').disabled);loseOfficialAcceptance=true;await page.locator('#tailscale-install-release').click();await confirm('Install / upgrade');assert.equal(writes.at(-1).url,'/api/software/tailscale/install');assert.equal(writes.at(-1).body.version,'1.104.5');await page.waitForFunction(()=>document.querySelector('#tailscale-installer').textContent.includes('Up to date'));loseOfficialAcceptance=false;assert.equal(writes.filter(w=>w.url==='/api/software/tailscale/install').length,1);
  ts.client_version='1.104.4';await page.locator('#tailscale-service').getByRole('button',{name:'Refresh status',exact:true}).click();await page.waitForFunction(()=>!document.querySelector('#tailscale-install-release').disabled);
  const beforeStaleInstall=writes.length;await page.locator('#tailscale-install-release').click();await page.evaluate(()=>OpenU60Navigation.go('network/proxy'));await confirm('Install / upgrade');assert.equal(writes.length,beforeStaleInstall);await page.evaluate(()=>OpenU60Navigation.go('network/tailscale'));
  // A pending confirmation is invalidated by leaving the page.
  ts.updating=false;await page.locator('#tailscale-service').getByRole('button',{name:'Refresh status',exact:true}).click();await page.waitForFunction(()=>!document.querySelector('#tailscale-policy button[type=submit]').disabled);
  const before=writes.length;await page.getByRole('button',{name:'Stop Tailscale',exact:true}).click();await page.evaluate(()=>OpenU60Navigation.go('network/proxy'));await confirm('Stop Tailscale');assert.equal(writes.length,before);
  // A late authenticated reply cannot repopulate private state after lock.
  holdStatus=true;await page.locator('#proxy-service').getByRole('button',{name:'Refresh status',exact:true}).click();await page.waitForTimeout(100);
  holdRelease=true;await page.getByRole('tab',{name:'Installation',exact:true}).click();await page.locator('#mihomo-check-update').click();await page.waitForTimeout(100);
  holdConfig=true;await page.getByRole('tab',{name:'Configuration',exact:true}).click();await page.waitForTimeout(100);await page.locator('.proxy-import-options summary').click();await page.locator('#proxy-config').fill('private draft');await page.locator('#proxy-source-url').fill('private subscription');await page.evaluate(()=>window.dispatchEvent(new Event('dashboard:locked')));releaseStatus?.();releaseMetadata?.();releaseConfig?.();await page.waitForTimeout(100);
  assert.match(await page.locator('#mihomo-installer').textContent(),/Not checked/);assert.equal(await page.locator('#mihomo-install-release').isDisabled(),true);
  assert.equal(await page.locator('#proxy-config').inputValue(),'');assert.equal(await page.locator('#proxy-source-url').inputValue(),'');assert.equal(await page.locator('#proxy-service .service-summary').textContent(),'Waiting for status');assert.equal(writes.length,before);assert.deepEqual(errors,[]);
  holdStatus=false;holdConfig=false;holdRelease=false;
  for(const language of ['zh-Hans','zh-Hant','ja','ko']){
   await page.locator('.topbar [data-language]').selectOption(language);await page.waitForFunction(expected=>document.documentElement.lang===expected&&OpenU60Console.connected,language);
   await page.locator('#proxy-tab-configuration').click();await page.waitForFunction(()=>document.querySelector('#proxy-config').value.includes('changed elsewhere'));
   const draft=savedConfig+'\n# locale '+language;await page.locator('#proxy-config').fill(draft);await page.locator('#proxy-save-config').click();await confirm(await page.evaluate(()=>OpenU60I18n.t('Apply configuration')));
   assert.equal(writes.at(-1).url,'/api/proxy/mihomo/config');assert.equal(writes.at(-1).body.config,draft);assert.equal(savedConfig,draft);assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);
  }
  assert.deepEqual(errors,[]);
  console.log('remote services: policy, import, installation, node selection, Tailscale, stale-session protection and responsive layouts passed');
 }finally{for(const stream of streams)stream.end();await browser.close();await new Promise(r=>server.close(r));}
})().catch(e=>{console.error(e);process.exitCode=1;});
