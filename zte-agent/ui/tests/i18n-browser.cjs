'use strict';
// Synthetic data only; language changes must never mutate router configuration.
const {chromium}=require('playwright'),assert=require('node:assert/strict'),http=require('node:http'),fs=require('node:fs'),path=require('node:path');
(async()=>{
 const root=path.join(__dirname,'..'),ok=data=>({ok:true,data}),writes=[],streams=new Set(),errors=[];
 const wifi={wifi_onoff:'1',disabled_2g:'0',disabled_5g:'0',ssid_2g:'Network {0} 中文',ssid_5g:'Network 5',encryption_2g:'sae-mixed',encryption_5g:'sae-mixed',hidden_2g:'0',hidden_5g:'0',txpower_2g:'100',txpower_5g:'100',channel_2g:'auto',channel_5g:'auto',htmode_2g:'EHT20',htmode_5g:'EHT80'};
 const readings={device:ok({hostname:'Example router',uptime_secs:100}),wan:ok({up:true}),wan6:ok({up:false}),signal:ok({network_type:'5G',net_select:'Only_5G'}),modem:ok({operate_mode:'ONLINE'}),battery:ok({capacity:80}),wifi:ok(wifi),clients:ok({hosts:{}}),speed:ok({rx_speed:0,tx_speed:0})};
 const server=http.createServer((req,res)=>{
  if(req.url.startsWith('/api/dashboard/events')){res.writeHead(200,{'Content-Type':'text/event-stream'});streams.add(res);res.on('close',()=>streams.delete(res));res.write('event: snapshot\ndata: '+JSON.stringify({readings})+'\n\n');return;}
  if(req.url.startsWith('/api/')){if(req.method!=='GET')writes.push(req.url);res.setHeader('Content-Type','application/json');res.end(JSON.stringify(ok(req.url==='/api/wifi/status'?wifi:req.url==='/api/network/signal'?readings.signal.data:{})));return;}
  const name=req.url.split('?')[0]==='/'?'index.html':/^\/console\/a\/[\w.-]+$/.test(req.url)?req.url.split('/').pop():null;
  if(!name||!fs.existsSync(path.join(root,name))){res.writeHead(404);res.end();return;}res.setHeader('Content-Type',name.endsWith('.js')?'text/javascript':name.endsWith('.css')?'text/css':'text/html');res.end(fs.readFileSync(path.join(root,name)));
 });
 await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));const base='http://127.0.0.1:'+server.address().port,browser=await chromium.launch({headless:true,channel:'chrome'});
 try{
  for(const [language,code] of [['zh-CN','zh-Hans'],['zh-TW','zh-Hant'],['en-US','en'],['ja-JP','ja'],['ko-KR','ko']]){
   const context=await browser.newContext({locale:language,viewport:{width:390,height:844}}),page=await context.newPage();page.on('pageerror',e=>errors.push(code+': '+e.message));
   await page.goto(base+'/#network/wifi');await page.waitForFunction(()=>OpenU60Console.connected&&document.querySelector('#wifi-config-ssid_2g')&&!document.querySelector('#wifi-config-ssid_2g').disabled);
   assert.equal(await page.locator('html').getAttribute('lang'),code);
   assert.equal(await page.locator('#wifi-config-ssid_2g').inputValue(),wifi.ssid_2g);
   assert.equal(await page.locator('#wifi-config-encryption_2g').inputValue(),'sae-mixed');
   await page.locator('#wifi-config-ssid_2g').fill('Do not translate {1}');
   const next=code==='en'?'zh-Hant':'en';await page.locator('.topbar [data-language]').selectOption(next);await page.locator('.confirm-dialog').waitFor({state:'visible'});
   await page.locator('.confirm-dialog button').first().click();await page.waitForFunction(()=>!document.querySelector('.confirm-dialog').open);
   assert.equal(await page.locator('html').getAttribute('lang'),code);assert.equal(await page.locator('#wifi-config-ssid_2g').inputValue(),'Do not translate {1}');
   await page.locator('.topbar [data-language]').selectOption(next);await page.locator('.confirm-dialog').waitFor({state:'visible'});await page.locator('.confirm-dialog button').last().click();
   await page.waitForFunction(expected=>document.documentElement.lang===expected,next);await page.waitForFunction(()=>OpenU60Console.connected);
   assert.match(page.url(),/#network\/wifi$/);assert.equal(await page.evaluate(()=>localStorage.getItem('openu60.language')),next);
   await page.locator('.topbar [data-language]').selectOption(code);await page.waitForFunction(expected=>document.documentElement.lang===expected,code);
   await page.evaluate(()=>OpenU60Navigation.go('network/mobile'));await page.waitForFunction(()=>document.querySelector('#network-mode-config-net_select')?.options.length>0);
   assert.deepEqual(await page.locator('#network-mode-config-net_select option').evaluateAll(nodes=>nodes.map(n=>n.value)),['WL_AND_5G','LTE_AND_5G','Only_5G','Only_LTE','WCDMA_AND_LTE','Only_WCDMA']);
   await page.evaluate(()=>OpenU60Navigation.go('settings/appearance'));await page.waitForFunction(()=>OpenU60Navigation.current.key==='settings/appearance');await page.waitForTimeout(200);
   assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1),code+' mobile overflow');
   await page.screenshot({path:'/tmp/u60-web-language-'+code+'.png'});
   await page.setViewportSize({width:1280,height:900});await page.waitForTimeout(200);assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1),code+' desktop overflow');
   await context.close();
  }
  assert.deepEqual(writes,[]);assert.deepEqual(errors,[]);console.log('i18n browser: five locales, persistent switching, draft cancellation, raw values, protocol enums and responsive layouts passed');
 }finally{await browser.close();for(const stream of streams)stream.destroy();await new Promise(resolve=>server.close(resolve));}
})().catch(error=>{console.error(error);process.exitCode=1;});
