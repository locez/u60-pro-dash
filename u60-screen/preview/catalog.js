/* UI capability model. API references describe future integration, never requests.
 * Values below are fictional fixtures; preview.js is the only state owner. */
'use strict';
(() => {
  const groups = [
    ['wifi','Wi-Fi 与局域网','热点、访客与连接设备','network-menu','wifi'],
    ['mobile','移动网络','选网、APN 与流量套餐','network-menu','network'],
    ['radio','信号与锁频','载波、小区与无线诊断','network-menu','network'],
    ['routing','路由与防火墙','转发、访问规则与 QoS','network-menu','network'],
    ['dns','DNS 与隐私','解析、加密 DNS 与缓存','network-menu','network'],
    ['tailscale','Tailscale','回家通道与远程连接','apps','home'],
    ['sms','短信','收件箱、发送与自动转发','apps','message'],
    ['sim','SIM 与 eSIM','卡片安全、套餐与配置文件','apps','sim'],
    ['voice','通话与卡片服务','拨号、USSD 与 SIM 菜单','apps','phone'],
    ['tools','诊断工具','测速、进程与 AT 终端','apps','tools'],
    ['automation','自动化','计划任务与实时活动','apps','clock'],
    ['power','电池与电源','充电、休眠与 USB','device-settings','power'],
    ['system','系统设置','屏幕、HTTPS 与设备维护','device-settings','device'],
  ].map(([id,title,subtitle,root,icon])=>({id,title,subtitle,root,icon}));
  const features = [];
  const text=(key,label,value='',extra={})=>({key,label,value,type:'text',...extra});
  const num=(key,label,value,min=0,max=999999)=>({key,label,value,type:'number',min,max});
  const toggle=(key,label,value)=>({key,label,value,type:'toggle'});
  const select=(key,label,value,options)=>({key,label,value,type:'select',options});
  const multi=(key,label,value,options)=>({key,label,value,type:'multi',options});
  const action=(label,effect='action',extra={})=>({label,effect,...extra});
  const add=(id,title,group,extra={})=>features.push({id,title,group,description:'',rows:[],fields:[],actions:[],apis:[],...extra});
  const rw=path=>['GET '+path,'PUT '+path];
  const netWarning='应用后蜂窝或局域网连接可能短暂中断。';

  for (const [id,band,suffix,channels,widths] of [['wifi2','2.4 GHz','2g',['自动','1','6','11'],['20 MHz','40 MHz']],['wifi5','5 GHz','5g',['自动','36','44','149','161'],['20 MHz','40 MHz','80 MHz','160 MHz']]]) {
    add(id,band+' 热点','wifi',{description:'设置这一路无线网络。',apis:['GET /api/wifi/status','PUT /api/wifi/settings'],warning:'应用后连接到此热点的设备需要重新连接。',fields:[toggle('enabled','开启热点',true),text('ssid','网络名称',suffix==='2g'?'OpenU60':'OpenU60-5G',{required:true,maxLength:32}),select('security','安全方式','WPA2 / WPA3',['WPA2','WPA2 / WPA3','WPA3']),text('password','新密码','',{type:'password',placeholder:'留空保留原密码',minLength:8,maxLength:63}),select('channel','信道','自动',channels),select('width','信道带宽',widths.at(-1),widths),num('power','发射功率 %',100,1,100),toggle('hidden','隐藏网络名称',false)],actions:[action('保存热点设置','save')]});
  }
  add('guest','访客网络','wifi',{apis:rw('/api/wifi/guest'),description:'给访客提供单独的 Wi-Fi。',fields:[toggle('enabled','开启访客网络',false),text('ssid','网络名称','OpenU60-Guest',{required:true,maxLength:32}),text('password','新密码','',{type:'password',minLength:8,maxLength:63,placeholder:'留空保留原密码'}),select('security','加密方式','WPA2',['WPA2','WPA2 / WPA3']),toggle('isolation','隔离访客',true)],actions:[action('保存访客设置','save')]});
  add('clients','连接设备','wifi',{apis:['GET /api/network/clients','GET /api/network/client-details'],description:'查看连接方式、信号与设备流量。',kind:'list',items:[{title:'工作电脑',subtitle:'192.168.0.102 · 5 GHz',rows:[['在线时长','38 分钟'],['下载 / 上传','24.8 / 3.2 Mbps'],['链路速率','1,201 Mbps'],['信号','−45 dBm'],['限速','固件未报告'],['MAC','••:••:••:••:A1:02']]},{title:'随身手机',subtitle:'192.168.0.104 · 5 GHz',rows:[['在线时长','12 分钟'],['下载 / 上传','3.6 / 0.8 Mbps'],['信号','−52 dBm'],['已下载','128 MB']]},{title:'平板电脑',subtitle:'192.168.0.108 · 2.4 GHz',rows:[['在线时长','8 分钟'],['下载 / 上传','1.2 / 0.2 Mbps'],['信号','−57 dBm']]}],actions:[action('刷新设备','refresh')]});
  add('lan','LAN 与 DHCP','wifi',{apis:rw('/api/router/lan').concat('GET /api/network/lan-status'),warning:'修改网关地址后，需要通过新地址访问后台。',fields:[text('address','网关地址','192.168.0.1',{format:'ipv4',required:true}),text('mask','子网掩码','255.255.255.0',{format:'ipv4',required:true}),toggle('dhcp','DHCP 服务',true),num('start','地址池起始位',100,2,254),num('limit','地址池大小',100,1,253),text('lease','租约时间','12h',{required:true})],actions:[action('保存局域网设置','save')]});

  add('mobile-data','蜂窝数据','mobile',{apis:['GET /api/modem/status',...rw('/api/modem/data'),'POST /api/modem/airplane','POST /api/modem/online'],rows:[['数据连接','已连接'],['网络制式','5G SA']],fields:[toggle('data','移动数据',true),toggle('roaming','允许数据漫游',false),select('dial','连接方式','自动拨号',['自动拨号','手动拨号'])],warning:netWarning,actions:[action('保存连接设置','save'),action('开启飞行模式','airplane',{danger:true,confirm:'蜂窝数据、通话和短信将不可用。'}),action('恢复蜂窝联网','online')]});
  add('network-mode','首选网络','mobile',{apis:['PUT /api/modem/network-mode'],description:'根据覆盖情况选择制式。',fields:[select('mode','网络模式','自动 5G / 4G',['自动 5G / 4G','5G NSA','仅 5G SA','仅 4G LTE','4G / 3G','仅 3G'])],warning:netWarning,actions:[action('应用网络模式','save')]});
  add('operators','运营商选择','mobile',{apis:['POST /api/modem/scan','GET /api/modem/scan/status','GET /api/modem/scan/results','POST /api/modem/register','GET /api/modem/register/result'],description:'扫描附近网络并选择注册方式。',fields:[select('registration','注册方式','自动',['自动','手动']),select('operator','运营商','示例运营商 A',['示例运营商 A','示例运营商 B','示例运营商 C'])],rows:[['上次扫描','3 个可用网络']],actions:[action('扫描运营商','scan'),action('注册所选网络','save',{confirm:netWarning})]});
  add('apn','APN 配置','mobile',{apis:[...rw('/api/router/apn/mode'),...rw('/api/router/apn/profiles'),'POST /api/router/apn/profiles','GET /api/router/apn/auto-profiles','POST /api/router/apn/profiles/delete','POST /api/router/apn/profiles/activate'],kind:'list',description:'管理蜂窝数据接入点。',items:[{title:'运营商默认',subtitle:'internet · 当前使用',rows:[['IP 协议','IPv4 + IPv6'],['认证','无']],editable:true}],itemFields:[text('name','配置名称','',{required:true}),text('apn','APN','internet',{required:true}),select('protocol','IP 协议','IPv4 + IPv6',['IPv4','IPv6','IPv4 + IPv6']),select('auth','认证方式','无',['无','PAP','CHAP']),text('username','用户名'),text('password','密码','',{type:'password'})],itemActions:[action('设为当前 APN','activate',{confirm:netWarning})],actions:[action('添加 APN','add'),action('使用自动配置','automatic')]});
  add('usage','流量与套餐','mobile',{apis:['GET /api/data-usage','PUT /api/data-usage/limit','PUT /api/data-usage/cycle'],rows:[['本周期已用','12.48 GB'],['下载 / 上传','10.62 / 1.86 GB'],['计量来源','路由器统计']],description:'统计可能与运营商计费存在差异。',fields:[toggle('limit','套餐提醒',true),num('allowance','每月套餐 GB',100,1,10000),num('warning','提醒阈值 %',80,1,100),num('day','每月结算日',1,1,28)],actions:[action('保存套餐设置','save')]});
  add('oos','无服务省电','mobile',{apis:rw('/api/modem/oos-policy'),fields:[toggle('enabled','启用无服务省电',false),num('grace','等待时间 / 分钟',10,1,120),num('interval','搜索间隔 / 分钟',30,1,240),num('window','搜索窗口 / 分钟',2,1,30)],actions:[action('保存策略','save')]});
  add('watchdog','断网自动恢复','mobile',{apis:rw('/api/modem/conn-watchdog'),rows:[['当前状态','正常'],['探测来源','未配置']],description:'恢复策略需要有效的联网探测来源。',fields:[toggle('enabled','启用连接看护',false),toggle('reboot','允许最终重启恢复',false)],actions:[action('保存恢复策略','save')]});

  add('signal-live','实时信号','radio',{apis:['GET /api/network/signal'],rows:[['当前制式','5G SA'],['NR 频段','n78'],['RSRP','−86 dBm'],['SINR','21.4 dB'],['RSRQ','−9 dB'],['PCI','231'],['ARFCN','633984']],actions:[action('刷新信号','refresh')]});
  add('carriers','载波聚合','radio',{apis:['GET /api/network/signal'],kind:'list',items:[{title:'NR 主载波 · n78',subtitle:'100 MHz · PCI 231',rows:[['ARFCN','633984'],['RSRP','−86 dBm'],['SINR','21.4 dB']]},{title:'NR 辅载波 · n78',subtitle:'100 MHz · PCI 232',rows:[['ARFCN','641280'],['RSRP','−92 dBm'],['SINR','17.6 dB']]}]});
  for(const [id,title,api,options,value]of[['nr-bands','5G 频段偏好','nr',['n1','n3','n5','n8','n28','n41','n77','n78','n79'],['n41','n78']],['lte-bands','4G 频段偏好','lte',['B1','B3','B5','B7','B8','B20','B28','B38','B40','B41'],['B1','B3','B8']]])add(id,title,'radio',{apis:['POST /api/cell/band/'+api,'POST /api/cell/band/reset'],description:'点击选择允许使用的频段。',fields:[multi('bands','允许的频段',value,options)],warning:'频段限制可能导致无法注册网络。',actions:[action('应用频段偏好','save'),action('恢复自动选频','reset-fields')]});
  for(const [id,title,api]of[['nr-lock','5G 小区锁定','nr'],['lte-lock','4G 小区锁定','lte']])add(id,title,'radio',{apis:['POST /api/cell/lock/'+api,'POST /api/cell/lock/reset'],fields:[num('pci','PCI',231,0,api==='nr'?1007:503),num('channel',api==='nr'?'NR ARFCN':'LTE EARFCN',api==='nr'?633984:1775,0,3279165),text('band','频段',api==='nr'?'n78':'B3',{required:true})],warning:'锁定错误小区会失去蜂窝服务。',actions:[action('锁定小区','save'),action('解除小区锁定','unlock-cell')]});
  add('neighbors','邻区扫描','radio',{apis:['POST /api/cell/neighbors/scan','GET /api/cell/neighbors/nr','GET /api/cell/neighbors/lte'],kind:'list',items:[{title:'NR · n78 · PCI 232',subtitle:'−92 dBm · ARFCN 641280',rows:[['制式','5G NR'],['SINR','17.6 dB']]},{title:'LTE · B3 · PCI 265',subtitle:'−95 dBm · EARFCN 1775',rows:[['制式','4G LTE'],['SINR','14.2 dB']]}],actions:[action('重新扫描邻区','scan')]});
  add('stc','STC 小区策略','radio',{apis:[...rw('/api/cell/stc/params'),'GET /api/cell/stc/status','POST /api/cell/stc/enable','POST /api/cell/stc/disable','POST /api/cell/stc/reset'],rows:[['状态','未启用']],fields:[toggle('enabled','STC 小区策略',false),text('params','白名单参数','',{type:'textarea',placeholder:'固件支持的参数 JSON',format:'json'})],actions:[action('应用策略','save'),action('重置 STC','reset-fields',{danger:true})]});
  add('signal-test','信号质量检测','radio',{apis:['POST /api/cell/signal-detect/start','POST /api/cell/signal-detect/stop','GET /api/cell/signal-detect/results','GET /api/cell/signal-detect/progress'],kind:'progress',rows:[['RSRP 平均值','−87 dBm'],['SINR 平均值','20.8 dB']],actions:[action('开始检测','progress'),action('停止检测','stop-progress')]});

  add('wan','网络接口','routing',{apis:['GET /api/network/wan','GET /api/network/wan6','GET /api/network/rmnet','GET /api/network/lan-status'],rows:[['蜂窝接口','rmnet_data0'],['WAN IPv4','192.0.2.24'],['WAN IPv6','2001:db8::24'],['局域网网关','192.168.0.1'],['MTU','1400'],['连接状态','已连接']]});
  add('firewall','防火墙','routing',{apis:['GET /api/router/firewall','PUT /api/router/firewall/switch','PUT /api/router/firewall/level','GET /api/router/firewall/filter-rules'],fields:[toggle('enabled','开启防火墙',true),select('level','保护级别','中',['低','中','高'])],rows:[['过滤规则','3 条']],actions:[action('保存防火墙','save'),action('查看过滤规则','show-rules')]});
  add('nat','NAT 类型','routing',{apis:['GET /api/router/firewall','PUT /api/router/firewall/nat'],fields:[select('mode','NAT 模式','对称型',['对称型','全锥型'])],warning:netWarning,actions:[action('应用 NAT 模式','save')]});
  add('dmz','DMZ 主机','routing',{apis:['GET /api/router/firewall','PUT /api/router/firewall/dmz'],fields:[toggle('enabled','启用 DMZ',false),text('address','目标设备 IPv4','192.168.0.102',{format:'ipv4',required:true})],warning:'DMZ 会将未匹配其他规则的入站连接转给这台设备。',actions:[action('保存 DMZ','save')]});
  add('upnp','UPnP','routing',{apis:rw('/api/router/firewall/upnp'),fields:[toggle('enabled','允许自动端口映射',false)],rows:[['活动映射','0 条']],actions:[action('保存 UPnP','save')]});
  add('ports','端口转发','routing',{apis:['GET /api/router/firewall/port-forward','POST /api/router/firewall/port-forward','PUT /api/router/firewall/port-forward/switch'],kind:'list',items:[{title:'NAS 管理',subtitle:'TCP 5000 → 192.168.0.120:5000',editable:true}],itemFields:[text('name','规则名称','',{required:true}),select('protocol','协议','TCP',['TCP','UDP','TCP + UDP']),num('external','外部端口',5000,1,65535),text('address','内部地址','192.168.0.120',{format:'ipv4',required:true}),num('internal','内部端口',5000,1,65535),toggle('enabled','启用规则',true)],actions:[action('添加转发规则','add')]});
  add('qos','QoS 流量优先级','routing',{apis:rw('/api/router/qos'),fields:[toggle('enabled','启用 QoS',false),num('download','下载带宽 Mbps',300,1,10000),num('upload','上传带宽 Mbps',50,1,10000)],actions:[action('保存 QoS','save')]});
  add('vpn','VPN 与 ALG 穿透','routing',{apis:rw('/api/router/vpn'),fields:[toggle('pptp','PPTP 穿透',true),toggle('l2tp','L2TP 穿透',true),toggle('ipsec','IPsec 穿透',true),toggle('sip','SIP ALG',false)],actions:[action('保存穿透设置','save')]});
  add('ttl','TTL 与跳数','routing',{apis:rw('/api/router/ttl'),fields:[toggle('enabled','启用自定义跳数',false),num('ttl','IPv4 TTL',64,1,255),num('hop','IPv6 Hop Limit',64,1,255)],description:'用于蜂窝软件转发路径。',actions:[action('应用跳数设置','save')]});
  add('domain','域名过滤','routing',{apis:rw('/api/router/domain-filter'),fields:[toggle('enabled','启用域名过滤',false),select('mode','过滤方式','黑名单',['黑名单','白名单']),text('domains','域名列表','example.org',{type:'textarea',placeholder:'每行一个域名'})],actions:[action('保存过滤列表','save')]});
  add('dns-config','DNS 解析','dns',{apis:[...rw('/api/router/dns'),'GET /api/dns/status','POST /api/dns/test','PUT /api/dns/apply'],fields:[select('mode','解析方式','运营商自动',['运营商自动','手动 DNS','加密 DoH']),text('primary','首选 DNS','1.1.1.1',{format:'ipv4',required:true}),text('secondary','备用 DNS','8.8.8.8',{format:'ipv4',required:true})],actions:[action('测试解析','dns-test'),action('应用 DNS','save',{confirm:'DNS 更改会影响所有热点设备的域名解析。'})]});
  add('doh','加密 DNS','dns',{apis:['GET /api/doh/status','PUT /api/doh/config','POST /api/doh/enable','POST /api/doh/disable'],fields:[toggle('enabled','开启 DoH',false),text('url','DoH 地址','https://cloudflare-dns.com/dns-query',{type:'url',required:true}),text('bootstrap','引导 DNS','1.1.1.1',{format:'ipv4',required:true})],actions:[action('保存加密 DNS','save')]});
  add('dns-cache','DNS 缓存','dns',{apis:['GET /api/dns/cache','POST /api/dns/cache/clear','GET /api/doh/cache','POST /api/doh/cache/clear'],kind:'list',items:[{title:'example.org',subtitle:'A · 192.0.2.10 · TTL 120s',rows:[['状态','有效缓存']]},{title:'example.net',subtitle:'AAAA · 2001:db8::10 · TTL 60s',rows:[['状态','有效缓存']]}],actions:[action('刷新缓存列表','refresh'),action('清空缓存','clear-list',{confirm:'清除现有解析缓存，后续访问会重新查询。'})]});

  window.U60Catalog={groups,features,helpers:{text,num,toggle,select,multi,action,add,rw}};
})();
