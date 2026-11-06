const {chromium}=require('playwright');
const fs=require('node:fs'),path=require('node:path'),http=require('node:http'),assert=require('node:assert/strict');
(async()=>{
 const root=path.resolve(__dirname,'../../preview'),errors=[];
 const server=http.createServer((req,res)=>{const name=req.url==='/'?'index.html':req.url.split('?')[0].slice(1);if(!/^[\w.-]+$/.test(name)){res.writeHead(404);return res.end();}const file=path.join(root,name);if(!fs.existsSync(file)){res.writeHead(404);return res.end();}res.setHeader('Content-Type',name.endsWith('.js')?'text/javascript':name.endsWith('.css')?'text/css':'text/html');res.end(fs.readFileSync(file));});
 await new Promise(r=>server.listen(0,'127.0.0.1',r));const browser=await chromium.launch({channel:'chrome',headless:true});
 try{const page=await browser.newPage({viewport:{width:1200,height:900}});page.on('pageerror',e=>errors.push(e.message));await page.goto('http://127.0.0.1:'+server.address().port+'/');await page.waitForFunction(()=>!!window.U60Preview);
 await page.evaluate(()=>U60Preview.features.root('network-menu'));const catalog=page.locator('#catalog-content');
 await catalog.getByRole('button',{name:/VPN 与组网/}).click();
 assert.deepEqual(await catalog.locator('.menu-link b').allTextContents(),['Tailscale','WireGuard']);
 await catalog.getByRole('button',{name:/Tailscale/}).click();assert.equal(await catalog.locator('h2').textContent(),'Tailscale');
 assert.deepEqual(await catalog.locator('.menu-link b').allTextContents(),['连接与登录','网络设置','连通性检测','安装与更新']);
 for(const title of ['连接与登录','网络设置','安装与更新']){
  await catalog.getByRole('button',{name:new RegExp(title)}).click();assert.equal(await catalog.locator('h2').textContent(),title);
  await catalog.getByRole('button',{name:'返回',exact:true}).click();assert.equal(await catalog.locator('h2').textContent(),'Tailscale');
 }
 await catalog.getByRole('button',{name:/连通性检测/}).click();assert.equal(await page.locator('#tailnet-heading').textContent(),'连通性检测');
 await page.locator('#page-tailnet .back-button').click();assert.equal(await catalog.locator('h2').textContent(),'Tailscale');
 await catalog.getByRole('button',{name:'返回',exact:true}).click();assert.equal(await catalog.locator('h2').textContent(),'VPN 与组网');
 await catalog.getByRole('button',{name:/WireGuard/}).click();assert.equal(await catalog.locator('h2').textContent(),'WireGuard');
 await catalog.getByRole('button',{name:'返回',exact:true}).click();assert.equal(await catalog.locator('h2').textContent(),'VPN 与组网');
 await page.evaluate(()=>document.querySelector('.screen-nav [data-page="network"]').click());
 await page.locator('.tailnet-shortcut').click();assert.equal(await catalog.locator('h2').textContent(),'Tailscale');
 await page.evaluate(()=>U60Preview.features.root('apps'));assert.doesNotMatch(await catalog.textContent(),/Tailscale|WireGuard/);
 await page.evaluate(()=>U60Preview.features.group('proxy'));await catalog.getByRole('button',{name:/mihomo/}).click();assert.match(await catalog.textContent(),/运行与配置/);
 await page.evaluate(()=>U60Preview.features.feature('proxy-service'));assert.match(await catalog.textContent(),/运行与配置/);await catalog.getByRole('button',{name:'返回',exact:true}).click();assert.match(await catalog.textContent(),/代理组/);await catalog.getByRole('button',{name:'返回',exact:true}).click();assert.equal(await catalog.locator('h2').textContent(),'代理');await catalog.getByRole('button',{name:'返回',exact:true}).click();assert.equal(await catalog.locator('h2').textContent(),'网络');
 assert.deepEqual(errors,[]);console.log('Native preview navigation: network services, nested pages and back stack passed.');
 }finally{await browser.close();await new Promise(r=>server.close(r));}
})().catch(e=>{console.error(e);process.exitCode=1;});
