'use strict';
// Authenticated synthetic data only. Every write terminates at this local fixture.
const {chromium}=require('playwright'),assert=require('node:assert/strict'),http=require('node:http'),fs=require('node:fs'),path=require('node:path');
(async()=>{
 const root=path.join(__dirname,'..'),ok=data=>({ok:true,data}),writes=[],streams=new Set(),errors=[],requests=[];
 const token='fixture-language-session',password='fixture-language-password';
 let logins=0;
 const wifi={wifi_onoff:'1',disabled_2g:'0',disabled_5g:'0',ssid_2g:'Network {0} 中文',ssid_5g:'Network 5',encryption_2g:'sae-mixed',encryption_5g:'sae-mixed',hidden_2g:'0',hidden_5g:'0',txpower_2g:'100',txpower_5g:'100',channel_2g:'auto',channel_5g:'auto',htmode_2g:'EHT20',htmode_5g:'EHT80'};
 const readings={device:ok({hostname:'Example router',uptime_secs:100}),wan:ok({up:true,'ipv4-address':[{address:'192.0.2.42'}]}),wan6:ok({up:false}),signal:ok({network_provider:'Example Mobile',network_type:'5G',net_select:'Only_5G'}),modem:ok({operate_mode:'ONLINE'}),battery:ok({capacity:80}),wifi:ok(wifi),clients:ok({hosts:{}}),speed:ok({rx_speed:1250000,tx_speed:125000})};
 const send=(res,data,status=200)=>{res.writeHead(status,{'Content-Type':'application/json'});res.end(JSON.stringify(status===200?ok(data):{ok:false,error:'unauthorized'}));};
 const server=http.createServer((req,res)=>{
  if(req.url==='/api/auth/login'&&req.method==='POST'){
   let raw='';req.on('data',chunk=>raw+=chunk);req.on('end',()=>{if(JSON.parse(raw).password!==password){send(res,{},401);return;}logins++;send(res,{token});});return;
  }
  if(req.url.startsWith('/api/')){
   requests.push({url:req.url,method:req.method,authorization:req.headers.authorization});
   if(req.headers.authorization!==`Bearer ${token}`){send(res,{},401);return;}
  }
  if(req.url.startsWith('/api/dashboard/events')){res.writeHead(200,{'Content-Type':'text/event-stream'});streams.add(res);res.on('close',()=>streams.delete(res));res.write('event: snapshot\ndata: '+JSON.stringify({readings})+'\n\n');return;}
  if(req.url.startsWith('/api/')){
   if(req.method!=='GET'){let raw='';req.on('data',chunk=>raw+=chunk);req.on('end',()=>{const body=JSON.parse(raw);writes.push({url:req.url,method:req.method,body});send(res,body);});return;}
   send(res,req.url==='/api/wifi/status'?wifi:req.url==='/api/network/signal'?readings.signal.data:{});return;
  }
  const name=req.url.split('?')[0]==='/'?'index.html':/^\/console\/a\/[\w.-]+$/.test(req.url)?req.url.split('/').pop():null;
  if(!name||!fs.existsSync(path.join(root,name))){res.writeHead(404);res.end();return;}res.setHeader('Content-Type',name.endsWith('.js')?'text/javascript':name.endsWith('.css')?'text/css':'text/html');res.end(fs.readFileSync(path.join(root,name)));
 });
 await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));const base='http://127.0.0.1:'+server.address().port,browser=await chromium.launch({headless:true,channel:'chrome'});
 try{
  for(const [language,code] of [['en-US','en'],['zh-CN','zh-Hans'],['zh-TW','zh-Hant'],['ja-JP','ja'],['ko-KR','ko']]){
   const context=await browser.newContext({locale:language,viewport:{width:390,height:844}}),page=await context.newPage();page.on('pageerror',e=>errors.push(code+': '+e.message));
   page.setDefaultTimeout(8000);
   await page.goto(base+'/#network/wifi');await page.waitForFunction(()=>document.querySelector('#login-dialog').open);
   await page.locator('#password').fill(password);await page.locator('#login-form button[type=submit]').click();
   await page.waitForFunction(()=>OpenU60Console.connected&&document.querySelector('#wifi-config-ssid_2g')&&!document.querySelector('#wifi-config-ssid_2g').disabled);
   assert.equal(logins,writes.length+1,'one login per locale context');
   assert.equal(await page.locator('html').getAttribute('lang'),code);
   assert.equal(await page.locator('#wifi-config-ssid_2g').inputValue(),wifi.ssid_2g);
   assert.equal(await page.locator('#wifi-config-encryption_2g').inputValue(),'sae-mixed');
   await page.locator('#wifi-config-ssid_2g').fill('Do not translate {1}');
   const next=code==='en'?'zh-Hant':'en';await page.locator('.topbar [data-language]').selectOption(next);await page.locator('.confirm-dialog').waitFor({state:'visible'});
   await page.locator('.confirm-dialog button').first().click();await page.waitForFunction(()=>!document.querySelector('.confirm-dialog').open);
   assert.equal(await page.locator('html').getAttribute('lang'),code);assert.equal(await page.locator('#wifi-config-ssid_2g').inputValue(),'Do not translate {1}');
   await page.locator('.topbar [data-language]').selectOption(next);await page.locator('.confirm-dialog').waitFor({state:'visible'});await page.locator('.confirm-dialog button').last().click();
   await page.waitForFunction(expected=>document.documentElement.lang===expected,next);await page.waitForFunction(()=>OpenU60Console.connected);
   assert.equal(await page.locator('#operator').textContent(),'Example Mobile');assert.equal(await page.locator('#wan-ip').textContent(),'192.0.2.42');assert.equal(await page.locator('#download').textContent(),'10.00');
   assert.equal(await page.evaluate(()=>sessionStorage.getItem('openu60.session')),token,'language reload retains the authenticated session');
   assert.match(page.url(),/#network\/wifi$/);assert.equal(await page.evaluate(()=>localStorage.getItem('openu60.language')),next);
   await page.locator('.topbar [data-language]').selectOption(code);await page.waitForFunction(expected=>document.documentElement.lang===expected,code);
   await page.waitForFunction(()=>OpenU60Console.connected);
   const body={ssid_2g:'Network 中文 {0}',encryption_2g:'sae-mixed',channel_2g:'auto'};
   assert.deepEqual(await page.evaluate(body=>OpenU60Console.request('/api/wifi/settings',{method:'PUT',body:JSON.stringify(body)}),body),body,'authenticated settings use raw protocol values');
   for(const stream of streams)stream.write('event: patch\ndata: '+JSON.stringify({readings:{signal:ok({network_type:'NO_SERVICE'})}})+'\n\n');
   await page.waitForFunction(()=>document.querySelector('.connection-panel').dataset.state==='offline');
   assert.equal(await page.locator('#connection-title').textContent(),await page.evaluate(()=>OpenU60I18n.t('Cellular service unavailable')));
   assert.equal(await page.evaluate(()=>OpenU60Console.connected),true,'no cellular service must not disconnect the console');
   for(const stream of streams)stream.write('event: patch\ndata: '+JSON.stringify({readings:{signal:readings.signal,speed:readings.speed}})+'\n\n');
   await page.waitForFunction(()=>document.querySelector('.connection-panel').dataset.state==='online');
   await page.evaluate(()=>OpenU60Navigation.go('network/mobile'));await page.waitForFunction(()=>document.querySelector('#network-mode-config-net_select')?.options.length>0);
   assert.deepEqual(await page.locator('#network-mode-config-net_select option').evaluateAll(nodes=>nodes.map(n=>n.value)),['WL_AND_5G','LTE_AND_5G','Only_5G','Only_LTE','WCDMA_AND_LTE','Only_WCDMA']);
   await page.evaluate(()=>OpenU60Navigation.go('settings/appearance'));await page.waitForFunction(()=>OpenU60Navigation.current.key==='settings/appearance');await page.waitForTimeout(200);
   assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1),code+' mobile overflow');
   await page.screenshot({path:'/tmp/u60-web-language-'+code+'.png'});
   await page.setViewportSize({width:1280,height:900});await page.waitForTimeout(200);assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1),code+' desktop overflow');
   await context.close();
  }
  assert.equal(logins,5);assert.equal(writes.length,5);assert.ok(writes.every(r=>r.url==='/api/wifi/settings'&&r.method==='PUT'));
  assert.ok(requests.filter(r=>r.authorization).every(r=>r.authorization===`Bearer ${token}`));assert.ok(requests.every(r=>!r.url.includes(token)));
  assert.deepEqual(errors,[]);console.log('i18n browser: five locales, authenticated streams and settings, session retention, persistent switching, draft cancellation, protocol values and responsive layouts passed');
 }finally{await browser.close();for(const stream of streams)stream.destroy();await new Promise(resolve=>server.close(resolve));}
})().catch(error=>{console.error(error);process.exitCode=1;});
