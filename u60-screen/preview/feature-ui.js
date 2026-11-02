/* Data-driven small-screen navigation and in-memory interaction simulator.
 * Deliberately has no HTTP client, credentials, shell bridge or device transport. */
'use strict';
window.createFeatureUI = function createFeatureUI(ctx) {
  const {groups,features}=window.U60Catalog;
  const byId=new Map(features.map(f=>[f.id,f]));
  const models=new Map();
  const host=document.getElementById('catalog-content');
  let route=null, stack=[], jobTimer=null, jobOwner=null, dialogResolve=null, dialogFocus=null;
  const clone=value=>JSON.parse(JSON.stringify(value));
  const node=(tag,cls,value)=>{const n=document.createElement(tag);if(cls)n.className=cls;if(value!==undefined)n.textContent=value;return n;};
  const icon=name=>{const svg=document.createElementNS('http://www.w3.org/2000/svg','svg');svg.setAttribute('class','icon');svg.setAttribute('aria-hidden','true');const use=document.createElementNS(svg.namespaceURI,'use');use.setAttribute('href','#i-'+name);svg.append(use);return svg;};
  const button=(label,handler,cls='secondary-button')=>{const b=node('button',cls,label);b.type='button';b.addEventListener('click',handler);return b;};
  const groupOf=f=>groups.find(g=>g.id===f.group);
  function model(f) {
    if(!models.has(f.id)){
      const values=Object.fromEntries(f.fields.map(v=>[v.key,clone(v.value)]));
      if(f.id==='screen-prefs')Object.assign(values,ctx.getScreenPreferences());
      models.set(f.id,{values,rows:clone(f.rows),items:clone(f.items||[]),message:'',error:false,progress:null,output:'',editors:{}});
    }
    return models.get(f.id);
  }
  function syncPreferences(patch){
    const m=models.get('screen-prefs');if(!m)return;
    Object.assign(m.values,patch);
    if(route?.type==='feature'&&route.id==='screen-prefs'){
      for(const [key,value]of Object.entries(patch)){
        const input=document.getElementById('field-screen-prefs-'+key);if(input)input.value=value;
      }
    }
  }
  function setRow(m,label,value){const row=m.rows.find(r=>r[0]===label);if(row)row[1]=value;else m.rows.push([label,value]);}
  function rootFor(r=route){if(!r)return 'apps';if(r.type==='root')return r.id;const g=r.type==='group'?groups.find(g=>g.id===r.id):groupOf(byId.get(r.id));return g.root==='device-settings'?'device':g.root;}
  function navigate(next,push=true){cancelDialog();if(push&&route)stack.push(route);route=next;ctx.showCatalog(rootFor());render();}
  function root(id){stack=[];navigate({type:'root',id},false);}
  function group(id){const g=groups.find(g=>g.id===id);if(!g)return;stack=[{type:'root',id:g.root}];navigate({type:'group',id},false);}
  function feature(id){const f=byId.get(id);if(!f)return;const g=groupOf(f);if(f.view){ctx.showStatic(f.view);return;}stack=[{type:'root',id:g.root},{type:'group',id:g.id}];navigate({type:'feature',id},false);}
  function back(){cancelDialog();if(stack.length){route=stack.pop();ctx.showCatalog(rootFor());render();}else ctx.showStatic('network');}
  function header(title,description,showBack=true){const wrap=node('header','catalog-head');if(showBack){const b=button('返回',back,'back-button');b.prepend(icon('back'));wrap.append(b);}else wrap.append(node('p','screen-overline','OPEN U60'));wrap.append(node('h2','',title));if(description)wrap.append(node('p','',description));host.append(wrap);}
  function menu(title,subtitle,handler,symbol,unread=false){const b=button('',handler,'menu-link');if(symbol)b.append(icon(symbol));const t=node('span');t.append(node('b','',title));if(subtitle)t.append(node('small','',subtitle));b.append(t);if(unread)b.append(node('i','unread-mark'));b.append(icon('arrow'));return b;}
  function rows(values){if(!values.length)return;const dl=node('dl','feature-rows');for(const [k,v]of values){const d=node('div');d.append(node('dt','',k),node('dd','',v));dl.append(d);}host.append(dl);}
  function feedback(m){if(!m.message)return;const p=node('p','feature-feedback'+(m.error?' error':''),m.message);p.setAttribute('role','status');host.append(p);}
  function message(f,text,error=false){const m=model(f);m.message=text;m.error=error;if(route?.id===f.id)render(false);ctx.toast(text);}
  function fields(specs,values,id){
    const form=node('form','feature-form');form.id='feature-form';form.autocomplete='off';
    form.addEventListener('submit',e=>e.preventDefault());
    for(const spec of specs){
      const wrapper=node('div','feature-field'+(spec.type==='toggle'?' field-toggle':''));
      const label=node('label','',spec.label);label.htmlFor='field-'+id+'-'+spec.key;
      if(spec.type==='multi'){
        label.removeAttribute('for');label.className='field-label';wrapper.append(label);
        const picker=node('div','band-picker');picker.setAttribute('role','group');picker.setAttribute('aria-label',spec.label);
        for(const option of spec.options){const b=button(option,()=>{const set=new Set(values[spec.key]);if(set.has(option))set.delete(option);else set.add(option);values[spec.key]=[...set];b.setAttribute('aria-pressed',String(set.has(option)));},'');b.setAttribute('aria-pressed',String(values[spec.key].includes(option)));picker.append(b);}wrapper.append(picker);
      }else{
        const input=node(spec.type==='select'?'select':spec.type==='textarea'?'textarea':'input');input.id=label.htmlFor;input.name=spec.key;
        if(spec.type==='select'){for(const option of spec.options){const o=node('option','',option);o.value=option;input.append(o);}}
        else if(spec.type!=='textarea')input.type=spec.type==='toggle'?'checkbox':spec.type;
        if(spec.type==='toggle')input.checked=!!values[spec.key];else input.value=values[spec.key]??'';
        for(const key of ['required','min','max','minLength','maxLength','placeholder','pattern','inputMode'])if(spec[key]!==undefined)input[key]=spec[key];
        if(spec.type==='password')input.autocomplete='new-password';
        input.addEventListener('input',()=>{values[spec.key]=spec.type==='toggle'?input.checked:input.value;});
        input.addEventListener('change',()=>{values[spec.key]=spec.type==='toggle'?input.checked:input.value;});
        wrapper.append(label,input);
      }
      form.append(wrapper);
    }
    host.append(form);return form;
  }
  function validate(specs,values){
    const form=document.getElementById('feature-form');if(form&&!form.reportValidity())return false;
    for(const f of specs){const value=values[f.key];const input=document.getElementById('field-'+route.id+'-'+f.key);let error='';
      if(f.required&&typeof value==='string'&&!value.trim())error='请填写'+f.label;
      if(f.format==='ipv4'&&value&&!(String(value).split('.').length===4&&String(value).split('.').every(v=>/^\d{1,3}$/.test(v)&&Number(v)<=255)))error='请输入有效的 IPv4 地址';
      if(f.format==='json'&&value){try{JSON.parse(value);}catch{error='请输入有效的 JSON';}}
      if(f.pattern&&value&&!new RegExp('^(?:'+f.pattern+')$').test(value))error='请检查'+f.label+'的格式';
      if(f.matches&&value!==values[f.matches])error='两次输入不一致';
      if(f.type==='multi'&&!value.length)error='至少选择一个频段';
      if(error){ctx.toast(error);if(input){input.setCustomValidity(error);input.reportValidity();input.addEventListener('input',()=>input.setCustomValidity(''),{once:true});}return false;}
    }
    return true;
  }
  const dialog=document.getElementById('screen-dialog');
  function setModal(on){dialog.hidden=!on;for(const el of document.querySelectorAll('#awake-ui > .screen-page,#awake-ui > .screen-nav'))el.inert=on;}
  function finishDialog(answer){setModal(false);const done=dialogResolve;dialogResolve=null;if(dialogFocus?.isConnected)dialogFocus.focus({preventScroll:true});done?.(answer);}
  function cancelDialog(){if(dialogResolve)finishDialog(false);}
  document.getElementById('dialog-cancel').addEventListener('click',()=>finishDialog(false));
  document.getElementById('dialog-confirm').addEventListener('click',()=>finishDialog(true));
  dialog.addEventListener('keydown',e=>{if(e.key==='Escape'){e.preventDefault();finishDialog(false);}if(e.key==='Tab'){e.preventDefault();const cancel=document.getElementById('dialog-cancel');const confirm=document.getElementById('dialog-confirm');(document.activeElement===cancel?confirm:cancel).focus();}});
  function confirm(title,body,danger=false){cancelDialog();dialogFocus=document.activeElement;document.getElementById('dialog-title').textContent=title;document.getElementById('dialog-body').textContent=body;document.getElementById('dialog-confirm').classList.toggle('danger',danger);setModal(true);document.getElementById('dialog-cancel').focus();return new Promise(resolve=>{dialogResolve=resolve;});}
  function render(restoreScroll=true){
    const page=document.getElementById('page-catalog');const oldScroll=page.scrollTop;host.replaceChildren();
    if(route.type==='root'){
      const names={'network-menu':['网络','从无线连接到路由规则。'],apps:['应用','设备里的通信与管理工具。'],'device-settings':['设备设置','电源、屏幕与系统维护。']};const [title,desc]=names[route.id];
      header(title,desc,false);
      for(const g of groups.filter(g=>g.root===route.id))host.append(menu(g.title,g.subtitle,()=>navigate({type:'group',id:g.id}),g.icon));
      host.append(node('p','catalog-counter','向上滑动，查看全部分类'));
    }else if(route.type==='group'){
      const g=groups.find(g=>g.id===route.id);const list=features.filter(f=>f.group===g.id);header(g.title,g.subtitle);host.append(node('p','catalog-counter',list.length+' 项功能'));
      for(const f of list)host.append(menu(f.title,f.external?'需要配套管理端':f.local?'屏幕本地设置':f.description,()=>{if(f.view)ctx.showStatic(f.view);else navigate({type:'feature',id:f.id});}));
    }else if(route.type==='item'){
      const f=byId.get(route.id),m=model(f),item=m.items[route.index];if(!item){back();return;}
      header(item.title,item.subtitle);feedback(m);if(item.body)host.append(node('div','message-body',item.body));rows(item.rows||[]);
      const actions=[...(item.editable?[{label:'编辑',effect:'edit-item'}]:[]),...(f.itemActions||[])];
      if(item.editable)actions.push({label:'删除',effect:'delete-item',danger:true});
      if(f.id==='stk')actions.push({label:'选择此服务',effect:'stk-select'});
      actionButtons(f,actions);if(!actions.length)host.append(button('返回列表',back));
    }else if(route.type==='edit'){
      const f=byId.get(route.id),m=model(f),key=route.index===null?'new':route.index;
      const current=m.items[route.index];
      if(!m.editors[key])m.editors[key]={...Object.fromEntries(f.itemFields.map(s=>[s.key,clone(s.value)])),...(current?.values||{}),...(current?{name:current.title}:{})};
      header(route.index===null?'新增'+f.title:'编辑'+f.title,'保存后在列表中查看。');feedback(m);fields(f.itemFields,m.editors[key],f.id);
      actionButtons(f,[{label:'保存',effect:'save-item'},{label:'取消',effect:'cancel-edit'}]);
    }else{
      const f=byId.get(route.id),m=model(f);header(f.title,f.description);feedback(m);
      if(f.kind==='list'){
        if(!m.items.length)host.append(node('p','empty-list','暂无记录'));
        m.items.forEach((item,index)=>host.append(menu(item.title,item.subtitle,()=>{item.unread=false;navigate({type:'item',id:f.id,index});},null,item.unread)));
      }
      rows(m.rows);
      if(f.guide){const list=node('ol','feature-guide');f.guide.forEach(t=>list.append(node('li','',t)));host.append(list);}
      if(f.fields.length)fields(f.fields,m.values,f.id);
      if(f.kind==='phone'){
        const keys=node('div','phone-keypad');for(const key of ['1','2','3','4','5','6','7','8','9','*','0','#'])keys.append(button(key,()=>{m.values.number=String(m.values.number||'')+key;document.getElementById('field-phone-number').value=m.values.number;},''));
        host.append(keys,button('删除一位',()=>{m.values.number=String(m.values.number||'').slice(0,-1);document.getElementById('field-phone-number').value=m.values.number;},'inline-feature-link'));
      }
      if(m.progress!==null){const box=node('div','job-progress');const p=node('progress');p.max=100;p.value=m.progress;box.append(node('p','',m.progress<100?'正在进行 · '+m.progress+'%':'已完成'),p);host.append(box);}
      if(m.output)host.append(node('pre','terminal-output',m.output));
      actionButtons(f,f.actions);
    }
    page.scrollTop=restoreScroll?0:oldScroll;
  }
  function actionButtons(f,actions){const wrap=node('div','feature-actions');actions.forEach((a,i)=>{const b=button(a.label,()=>run(f,a),((i===0&&!a.danger)?'primary-button':'secondary-button')+(a.danger?' danger':''));b.dataset.effect=a.effect;wrap.append(b);});host.append(wrap);}
  function startProgress(f,kind='progress'){
    if(jobOwner){const old=byId.get(jobOwner);model(old).message='任务已停止：开始了另一项演示任务。';}
    clearInterval(jobTimer);jobOwner=f.id;const m=model(f);m.progress=0;m.message='任务已开始（演示）。';m.error=false;render(false);
    jobTimer=setInterval(()=>{if(document.hidden||ctx.asleep())return;m.progress=Math.min(100,m.progress+10);if(m.progress===100){clearInterval(jobTimer);jobTimer=null;jobOwner=null;m.message='已完成（演示）。';if(kind==='speedtest'){setRow(m,'下载','286.4 Mbps');setRow(m,'上传','42.8 Mbps');setRow(m,'延迟','24 ms');}if(kind==='esim-install'){const profiles=model(byId.get('esim-profiles'));profiles.items.push({title:'新下载套餐',subtitle:'示例运营商 · 已停用',editable:true,rows:[['ICCID 尾号','0099'],['状态','已停用']]});}ctx.toast(m.message);}if(route?.id===f.id&&route.type==='feature')render(false);},350);
  }
  async function run(f,a){
    const m=model(f),effect=a.effect;
    if(!f.local&&!ctx.connected()&&!['resume-session','guide'].includes(effect)){message(f,'后台会话已过期，请先恢复演示会话。',true);return;}
    const validating=['save','send-sms','verify-pin','at','esim-preview','screen-prefs','factory-reset','dns-test','ussd','ussd-reply','dial','progress'];
    if(validating.includes(effect)&&!validate(f.fields,m.values))return;
    if(effect==='save-item'){
      const key=route.index===null?'new':route.index;if(!validate(f.itemFields,m.editors[key]))return;
    }
    if((a.confirm||a.danger||(effect==='save'&&f.warning))&&!await confirm(a.label,a.confirm||f.warning||'确认'+a.label+'？此处将演示操作后的界面。',a.danger))return;
    if(effect==='add'){m.message='';m.editors.new=null;navigate({type:'edit',id:f.id,index:null});return;}
    if(effect==='edit-item'){m.message='';m.editors[route.index]=null;navigate({type:'edit',id:f.id,index:route.index});return;}
    if(effect==='cancel-edit'){back();return;}
    if(effect==='save-item'){
      const key=route.index===null?'new':route.index,values=clone(m.editors[key]);const item=route.index===null?{}:m.items[route.index];
      item.title=values.name||'新项目';item.values=values;item.editable=true;item.subtitle=f.id==='ports'?`${values.protocol} ${values.external} → ${values.address}:${values.internal}`:f.id==='scheduler'?`${values.repeat} ${values.time} · ${values.enabled?'已启用':'已停用'}`:f.id==='sms-forward'?`${values.channel} · ${values.enabled?'已启用':'已停用'}`:f.id==='apn'?values.apn:item.subtitle;
      if(route.index===null)m.items.push(item);delete m.editors[key];route={type:'feature',id:f.id};stack=stack.filter(r=>r.type!=='item'&&r.type!=='edit');message(f,'已保存到演示列表。');return;
    }
    if(effect==='delete-item'){
      const item=m.items[route.index];if(f.id==='esim-profiles'&&item.subtitle.includes('使用中')){message(f,'请先切换到另一个配置，再删除当前使用的配置。',true);return;}
      m.items.splice(route.index,1);back();message(f,'已删除该演示项目。');return;
    }
    if(effect==='activate'){
      m.items.forEach((item,index)=>{item.subtitle=(item.subtitle||'').replace(/ · (使用中|已停用|当前使用)$/,'')+(index===route.index?' · 使用中':' · 已停用');});message(f,'已切换，连接已恢复（演示）。');return;
    }
    if(effect==='save'){
      m.saved=clone(m.values);for(const spec of f.fields.filter(s=>s.type==='password'))m.values[spec.key]='';
      if(f.id==='mobile-data')ctx.scenario(m.values.data?'normal':'no-internet');
      message(f,'设置已保存（仅本次预览）。');return;
    }
    if(effect==='reset-fields'){for(const field of f.fields)m.values[field.key]=field.type==='multi'?[...field.options]:clone(field.value);message(f,'已恢复自动设置（演示）。');return;}
    if(effect==='clear-list'){m.items=[];message(f,'记录已清空（演示）。');return;}
    if(effect==='refresh'){message(f,'刚刚更新 · 演示数据');return;}
    if(effect==='scan'){message(f,f.id==='operators'?'扫描完成：找到 3 个可用网络。':'扫描完成：找到 2 个邻区。');return;}
    if(effect==='compose'||effect==='reply'){const compose=model(byId.get('sms-compose'));if(effect==='reply')compose.values.number='10000';feature('sms-compose');return;}
    if(effect==='send-sms'){const inbox=model(byId.get('sms-inbox'));inbox.items.unshift({title:'已发送',subtitle:'刚刚 · '+m.values.number,body:m.values.body});m.values.body='';message(f,'短信已发送（演示）。');return;}
    if(effect==='esim-add'){feature('esim-download');return;}
    if(effect==='esim-preview'){
      if(await confirm('安装此 eSIM？','示例运营商 · 数据套餐。确认后开始演示下载与安装。')){m.values.activation='';m.values.confirmation='';startProgress(f,'esim-install');}return;
    }
    if(effect==='progress'||effect==='speedtest'){if(jobOwner===f.id){message(f,'这项任务正在进行。');return;}startProgress(f,effect);return;}
    if(effect==='stop-progress'){if(jobOwner===f.id){clearInterval(jobTimer);jobTimer=null;jobOwner=null;}message(f,'任务已停止（演示）。');return;}
    if(effect==='dismiss-progress'){m.progress=null;message(f,'已清除任务结果。');return;}
    if(effect==='at'){m.output='> '+m.values.command+'\n'+(m.values.command.toUpperCase()==='AT+CSQ'?'+CSQ: 24,99\n\nOK':'OK\n（模拟响应，未发送给调制解调器）');message(f,'收到模拟响应。');return;}
    if(effect==='clear-output'){m.output='';m.message='';render(false);return;}
    if(effect==='airplane'||effect==='online'){ctx.scenario(effect==='airplane'?'no-internet':'normal');setRow(m,'数据连接',effect==='airplane'?'飞行模式':'已连接');message(f,'已切换模拟网络状态。');return;}
    if(effect==='ts-enable'||effect==='ts-disable'||effect==='ts-logout'){const on=effect==='ts-enable';ctx.tailscale(on);setRow(m,'当前状态',on?'已连接':effect==='ts-logout'?'未登录':'已停止');message(f,on?'Tailscale 已连接（演示）。':'家庭内网连接已停止（演示）。');return;}
    if(effect==='ts-login'){if(await confirm('登录 Tailscale','实际设备会显示 agent 返回的登录链接，供手机完成授权。现在模拟授权完成？')){ctx.tailscale(true);setRow(m,'当前状态','已连接');message(f,'模拟授权完成。');}return;}
    if(effect==='screen-prefs'){const saved=ctx.appearance({theme:m.values.theme==='浅色'?'light':'dark',accent:m.values.accent});ctx.brightness(Number(m.values.brightness));ctx.autoSleep({'15 秒':15,'30 秒':30,'60 秒':60,'从不':0}[m.values.sleep]);message(f,saved?'已应用到预览，并记住明暗与强调色。':'已应用到预览；浏览器未允许保存配色。');return;}
    if(effect==='dial'||effect==='answer'||effect==='hangup'||effect==='mute'||effect==='dtmf'){
      if(effect==='dial'&&!m.values.number){message(f,'请先输入电话号码。',true);return;}
      const label={dial:'通话中',answer:'通话中',hangup:'已挂断',mute:m.muted?'通话中':'通话中 · 已静音',dtmf:'通话中'}[effect];if(effect==='mute')m.muted=!m.muted;setRow(m,'通话状态',label);message(f,effect==='dtmf'?'已发送演示按键音。':label+'（演示）');return;
    }
    if(effect.startsWith('https-')){setRow(m,'HTTPS',effect==='https-reset'?'未开启':'已开启');setRow(m,'证书',effect==='https-reset'?'尚未签发':'已签发 · 需在客户端安装');message(f,'证书状态已更新（演示）。');return;}
    if(effect==='reboot'){message(f,'设备已重新启动（演示）。');ctx.toast('模拟重启完成，网络已恢复');return;}
    if(effect==='factory-reset'){ctx.reset();ctx.toast('已模拟恢复出厂：预览数据已还原');return;}
    if(effect==='expire-session'){ctx.session(false);setRow(m,'连接状态','会话已过期');message(f,'会话已过期。实时数据已暂停。');return;}
    if(effect==='resume-session'){ctx.session(true);setRow(m,'连接状态','演示会话');message(f,'演示会话已恢复。');return;}
    if(effect==='unregister'){setRow(m,'注册设备','0 台');setRow(m,'活动状态','未绑定');message(f,'已取消演示绑定。');return;}
    if(effect==='receipts'){setRow(m,'待处理回执','0 条');message(f,'回执已处理（演示）。');return;}
    if(effect.startsWith('ussd')){setRow(m,'会话状态',effect==='ussd-cancel'?'已结束':'等待回复');m.output=effect==='ussd-cancel'?'':'示例运营商\n1. 套餐查询\n2. 余额查询\n请在输入框回复选项。';message(f,'USSD 会话已更新（演示）。');return;}
    const responses={'verify-pin':'SIM 已解锁（演示）。','dns-test':'解析测试成功 · 23 ms（演示）。','test-forward':'演示消息已转发。','retry':'暂存消息已重试（演示）。','unlock-cell':'已解除小区锁定（演示）。','automatic':'已恢复运营商自动 APN（演示）。','show-rules':'允许已建立连接；允许局域网访问；默认拒绝其他入站连接。','clean-processes':'已清理 3 个演示服务；核心网络服务保持运行。','stk-select':'卡片服务已打开，可返回列表填写回复。','stk-reply':'SIM 菜单已收到回复（演示）。','stk-back':'已返回 SIM 主菜单。','stk-cancel':'卡片会话已结束。','guide':'请按页面上的步骤，在配套客户端继续。'};
    message(f,responses[effect]||'操作完成（演示）。');
  }
  function reset(){clearInterval(jobTimer);jobTimer=null;jobOwner=null;models.clear();stack=[];route=null;cancelDialog();}
  function current(){return route?{...route}:null;}
  return {root,group,feature,back,reset,cancelDialog,confirm,current,model,features,render,syncPreferences};
};
