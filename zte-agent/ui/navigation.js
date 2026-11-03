'use strict';
(() => {
  const tr = globalThis.OpenU60I18n?.t || ((s,...v)=>s.replace(/\{(\d+)\}/g,(match,i)=>Number(i)<v.length?String(v[i]):match));
  const icons = {
    dashboard: '<rect x="3" y="3" width="18" height="18" rx="3"/><path d="M9 3v18M9 10h12"/>',
    network: '<path d="M3 9a15 15 0 0 1 18 0M6 13a10 10 0 0 1 12 0m-9 4a5 5 0 0 1 6 0"/><circle cx="12" cy="21" r=".5"/>',
    sms: '<path d="M21 11a8 8 0 0 1-8 8H7l-4 3V5a3 3 0 0 1 3-3h7a8 8 0 0 1 8 9Z"/><path d="M7 8h10M7 12h7"/>',
    tools: '<path d="m14 6 4-4a6 6 0 0 1-7 8L4 21l-3-3L12 7a6 6 0 0 1 8-7l-4 4 1 3 3 1"/>',
    settings: '<path d="m10 3-1 3-3 1-3 3 2 3-1 3 3 3 3-1 3 2 3-3 1-3 3-1-1-4-3-1-1-3Z"/><circle cx="12" cy="12" r="3"/>'
  };
  const groups = [
    {id:'dashboard',label:tr('Dashboard'),pages:[
      {id:'overview',label:tr('Overview'),title:tr('Router overview'),description:tr('A clear view of your connection and the devices on it.')},
      {id:'history',label:tr('Connection history'),title:tr('Connection history'),description:tr('Interruptions and recovery observed during this tab session.')}
    ]},
    {id:'network',label:tr('Network'),pages:[
      {id:'signal',label:tr('Signal & Cells'),title:tr('Signal & Cells'),description:tr('Every reported LTE and NR carrier, with clear SA and NSA status.')},
      {id:'wifi',label:tr('Wi-Fi'),title:tr('Wi-Fi networks'),description:tr('Your local wireless networks and radio status.')},
      {id:'clients',label:tr('Connected Devices'),title:tr('Connected devices'),description:tr('Your devices, live traffic, observed peaks and reported speed limits.')},
      {id:'dns',label:tr('DNS & Privacy'),title:tr('DNS & privacy'),description:tr('Choose how names are resolved, test encrypted DNS and recover safely from bad settings.')},
      {id:'dns-cache',label:tr('Live DNS Cache'),title:tr('Live DNS cache'),description:tr('Watch answers enter the router cache, count down and disappear when they expire.')},
      {id:'proxy',label:tr('Proxies'),title:tr('Proxy services'),description:tr('Independent services, each with its own configuration and controls.')},
      {id:'tailscale',label:tr('Tailscale'),title:tr('Tailscale'),description:tr('Remote access and private subnet connectivity.')},
      {id:'vpn',label:tr('WireGuard VPN'),title:tr('WireGuard VPN'),description:tr('Import a split-tunnel profile and inspect native kernel handshakes and traffic.')},
      {id:'lan',label:tr('LAN & DHCP'),title:tr('LAN & DHCP'),description:tr('The router’s local address and the addresses it assigns to your devices.')},
      {id:'firewall',label:tr('Firewall & DMZ'),title:tr('Firewall & DMZ'),description:tr('Control inbound protection, address translation and automatic port access.')},
      {id:'ports',label:tr('Port forwarding'),title:tr('Port forwarding'),description:tr('Direct specific incoming connections to a device on your local network.')},
      {id:'qos',label:tr('Quality of service'),title:tr('Quality of service'),description:tr('Manage the router’s built-in traffic prioritization.')},
      {id:'mobile',label:tr('Mobile Network'),title:tr('Mobile network'),description:tr('The cellular data connection and modem radio state.')},
      {id:'ttl',label:tr('Packet TTL'),title:tr('Packet TTL'),description:tr('Adjust outbound IPv4 TTL and IPv6 hop limit on the cellular software path.')},
      {id:'esim',label:tr('eSIM management'),title:tr('eSIM management'),description:tr('Profiles, activation codes and connection status for the eSIM card in your router.')},
      {id:'operators',label:tr('Network operators'),title:tr('Network operators'),description:tr('Find available mobile networks and choose automatic or manual registration.')},
      {id:'apn',label:tr('APN profiles'),title:tr('APN profiles'),description:tr('Manage the access-point profiles used for mobile data.')},
      {id:'bands',label:tr('Bands & Cell Lock'),title:tr('Bands & Cell Lock'),description:tr('Choose preferred LTE and NR bands or a specific serving cell.')}
    ]},
    {id:'sms',label:'SMS',pages:[
      {id:'inbox',label:tr('Messages'),title:tr('Messages'),description:tr('Read, reply to and manage messages on your router and SIM.')},
      {id:'compose',label:tr('Compose'),title:tr('New message'),description:tr('Send a text message through the router’s SIM.')},
      {id:'forwarding',label:tr('Forwarding rules'),title:tr('SMS forwarding'),description:tr('Route incoming messages to the destinations you choose.')},
      {id:'storage',label:tr('Message storage'),title:tr('Message storage'),description:tr('Capacity and unread message counts reported by the modem.')}
    ]},
    {id:'tools',label:tr('Tools'),pages:[
      {id:'device',label:tr('Device Info'),title:tr('Device information'),description:tr('System details reported by the on-device agent.')},
      {id:'battery',label:tr('Battery & Power'),title:tr('Battery & power'),description:tr('Battery condition, charging, runtime estimates and measured power.')},
      {id:'diagnostics',label:tr('System diagnostics'),title:tr('System diagnostics'),description:tr('Live resource use, temperatures, processes and interface details.')},
      {id:'radio',label:tr('Radio diagnostics'),title:tr('Radio diagnostics'),description:tr('Inspect neighboring cells and run a signal quality sweep.')},
      {id:'speedtest',label:tr('Internet speed test'),title:tr('Internet speed test'),description:tr('Measure the router’s internet connection, with live progress.')},
      {id:'data',label:tr('Data usage'),title:tr('Data usage'),description:tr('Track router-measured usage and set a monthly allowance, warning and billing cycle.')},
      {id:'terminal',label:tr('AT terminal'),title:tr('AT terminal'),description:tr('Send an explicit modem command and inspect its response.')},
      {id:'traffic',label:tr('Traffic monitor'),title:tr('Traffic monitor'),description:tr('Live transfer rates and interface totals, streamed from the router.')}
    ]},
    {id:'settings',label:tr('Settings'),pages:[
      {id:'device',label:tr('Device & Power'),title:tr('Device & power'),description:tr('Charging, power settings and device maintenance.')},
      {id:'https',label:tr('HTTPS & Install'),title:tr('HTTPS & install'),description:tr('Secure local access and install this console on your devices.')},
      {id:'appearance',label:tr('Appearance'),title:tr('Appearance'),description:tr('A comfortable view, in any light.')},
      {id:'connection',label:tr('Console connection'),title:tr('Console connection'),description:tr('Connection status and access to this local console.')},
      {id:'update',label:tr('Agent Update'),title:tr('Agent update'),description:tr('Signed Stable and Beta releases with automatic rollback protection.')},
      {id:'about',label:tr('About'),title:tr('About OpenU60'),description:tr('A local console for your ZTE U60 Pro.')}
    ]}
  ];
  const aliases = {overview:'dashboard/overview',cellular:'network/signal',wireless:'network/wifi',devices:'network/clients',system:'tools/device'};
  const remembered = new Map(), positions = new Map();
  let current;
  const root = document.documentElement;
  const primary = document.querySelector('#primary-navigation');
  const sidebarToggle = document.querySelector('#sidebar-toggle');
  const mobileHost = document.querySelector('#mobile-navigation-host');
  const navigationSlot = document.createComment(tr('Desktop navigation'));
  primary.before(navigationSlot);
  const mobileLayout = matchMedia('(max-width:767px), (max-width:1023px) and (max-height:500px) and (hover:none) and (pointer:coarse)');
  const desktopLayout = matchMedia('(min-width:768px)');
  const reducedMotion = matchMedia('(prefers-reduced-motion:reduce)');
  const navigationPreference = 'openu60.navigation.collapsed';
  const groupAnimations = new WeakMap();
  let navigationCollapsed = false, navigationMotionTimer;
  let viewportFrame;
  try { navigationCollapsed = localStorage.getItem(navigationPreference) === 'true'; } catch { /* Private browsing can block storage. */ }
  function setNavigationCollapsed(collapsed,{remember=true,animate=true}={}) {
    navigationCollapsed = !!collapsed;
    clearTimeout(navigationMotionTimer);
    if (animate && desktopLayout.matches && !reducedMotion.matches) {
      root.dataset.navigationMotion = 'true';
      navigationMotionTimer = setTimeout(()=>delete root.dataset.navigationMotion,320);
    } else delete root.dataset.navigationMotion;
    root.dataset.navigation = navigationCollapsed ? 'collapsed' : 'expanded';
    const action = navigationCollapsed ? tr('Expand navigation') : tr('Collapse navigation');
    sidebarToggle.setAttribute('aria-expanded',String(!navigationCollapsed));
    sidebarToggle.setAttribute('aria-label',action);
    sidebarToggle.title = action;
    if (remember) try { localStorage.setItem(navigationPreference,String(navigationCollapsed)); } catch { /* Keep the choice for this tab. */ }
  }
  function stopGroupAnimation(details) {
    const running = groupAnimations.get(details);
    if (!running) return;
    running.animation.cancel(); groupAnimations.delete(details);
    const children = details.querySelector('.nav-children');
    children.style.removeProperty('height'); children.style.removeProperty('opacity'); children.style.removeProperty('overflow');
  }
  function animateGroup(details,targetOpen) {
    const children = details.querySelector('.nav-children');
    if (reducedMotion.matches) { stopGroupAnimation(details); details.open = targetOpen; return; }
    const running = groupAnimations.get(details);
    const startHeight = details.open ? children.getBoundingClientRect().height : 0;
    running?.animation.cancel();
    if (targetOpen) details.open = true;
    const endHeight = targetOpen ? children.scrollHeight : 0;
    children.style.overflow = 'hidden';
    const animation = children.animate([
      {height:`${startHeight}px`,opacity:targetOpen ? 0 : 1,transform:targetOpen ? 'translateY(-5px)' : 'translateY(0)'},
      {height:`${endHeight}px`,opacity:targetOpen ? 1 : 0,transform:targetOpen ? 'translateY(0)' : 'translateY(-5px)'}
    ],{duration:targetOpen ? 230 : 180,easing:'cubic-bezier(.22,.8,.24,1)'});
    groupAnimations.set(details,{animation,targetOpen});
    animation.addEventListener('finish',()=>{
      if (groupAnimations.get(details)?.animation !== animation) return;
      if (!targetOpen) details.open = false;
      groupAnimations.delete(details);
      children.style.removeProperty('height'); children.style.removeProperty('opacity'); children.style.removeProperty('overflow');
    },{once:true});
  }
  setNavigationCollapsed(navigationCollapsed,{remember:false,animate:false});
  sidebarToggle.addEventListener('click',()=>setNavigationCollapsed(!navigationCollapsed));
  for (const group of groups) {
    const details = document.createElement('details'); details.className = 'nav-group'; details.dataset.group = group.id;
    const summary = document.createElement('summary');
    // Icons are static application assets; page and modem values are never parsed as markup.
    summary.innerHTML = `<span class="nav-icon" aria-hidden="true"><svg viewBox="0 0 24 24">${icons[group.id]}</svg></span><span class="nav-text">${group.label}</span><span class="nav-chevron" aria-hidden="true">›</span>`;
    summary.setAttribute('aria-label',group.label); summary.title = group.label;
    const children = document.createElement('div'); children.className = 'nav-children';
    for (const page of group.pages) {
      const link = document.createElement('a'); link.href = `#${group.id}/${page.id}`; link.textContent = page.label; link.dataset.route = `${group.id}/${page.id}`;
      children.append(link);
    }
    summary.addEventListener('click', event => {
      if (mobileLayout.matches) {
        event.preventDefault(); go(remembered.get(group.id) || `${group.id}/${group.pages[0].id}`);
      } else if (navigationCollapsed) {
        event.preventDefault(); go(remembered.get(group.id) || `${group.id}/${group.pages[0].id}`);
      } else {
        event.preventDefault();
        const running = groupAnimations.get(details);
        animateGroup(details,running ? !running.targetOpen : !details.open);
      }
    });
    details.append(summary,children); primary.append(details);
  }
  function resolve(hash) {
    const key = aliases[hash] || hash;
    for (const group of groups) {
      const page = group.pages.find(page => `${group.id}/${page.id}` === key);
      if (page) return {group,page,key};
    }
    return {group:groups[0],page:groups[0].pages[0],key:'dashboard/overview'};
  }
  function apply(initial = false) {
    if (current) positions.set(current.key, scrollY);
    current = resolve(location.hash.slice(1));
    remembered.set(current.group.id,current.key);
    document.documentElement.dataset.route = current.key;
    document.title = tr("{0} · OpenU60", current.page.title);
    const title = document.querySelector('#page-title');
    title.textContent = current.page.title;
    const dot = document.createElement('span'); dot.textContent = '.'; title.append(dot);
    document.querySelector('#page-description').textContent = current.page.description;
    document.querySelector('#page-section').textContent = current.group.label.toUpperCase();
    document.querySelector('#breadcrumb-section').textContent = current.group.label;
    document.querySelector('#breadcrumb-page').textContent = current.page.label;
    for (const details of primary.querySelectorAll('.nav-group')) {
      const active = details.dataset.group === current.group.id;
      details.classList.toggle('active',active);
      const summary = details.querySelector('summary');
      if (active) { stopGroupAnimation(details); details.open = true; summary.setAttribute('aria-current','true'); }
      else summary.removeAttribute('aria-current');
      for (const link of details.querySelectorAll('a')) {
        const selected = link.dataset.route === current.key;
        link.classList.toggle('active',selected);
        if (selected) link.setAttribute('aria-current','page'); else link.removeAttribute('aria-current');
      }
    }
    const subtabs = document.querySelector('#section-tabs'); subtabs.replaceChildren();
    const picker = document.createElement('label'); picker.className = 'section-tabs-picker';
    const pickerLabel = document.createElement('span'); pickerLabel.className = 'section-tabs-picker-label';
    pickerLabel.textContent = tr("{0} pages", current.group.label);
    const pickerControl = document.createElement('span'); pickerControl.className = 'section-tabs-picker-control';
    const select = document.createElement('select'); select.id = 'section-page-picker';
    select.setAttribute('aria-label',tr("{0} pages", current.group.label));
    const links = document.createElement('div'); links.className = 'section-tab-links';
    for (const page of current.group.pages) {
      const route = `${current.group.id}/${page.id}`;
      const option = document.createElement('option'); option.value = route; option.textContent = page.label;
      option.selected = page.id === current.page.id; select.append(option);
      const link = document.createElement('a'); link.href = `#${current.group.id}/${page.id}`;
      link.textContent = page.label;
      if (page.id === current.page.id) link.setAttribute('aria-current','page');
      links.append(link);
    }
    select.addEventListener('change',() => go(select.value));
    pickerControl.append(select); picker.append(pickerLabel,pickerControl); subtabs.append(picker,links);
    for (const panel of document.querySelectorAll('[data-views]')) {
      panel.dataset.viewHidden = String(!panel.dataset.views.split(' ').includes(current.key));
    }
    window.dispatchEvent(new CustomEvent('dashboard:navigate',{detail:{group:current.group.id,page:current.page.id,key:current.key}}));
    if (!initial) {
      scrollTo({top:positions.get(current.key) || 0,behavior:'instant'});
      title.focus({preventScroll:true});
    }
    revealCurrentPage();
  }
  function revealCurrentPage() {
    const tabs = document.querySelector('.section-tab-links'), selected = tabs?.querySelector('[aria-current]');
    if (!selected || !tabs.clientWidth) return;
    const left = selected.getBoundingClientRect().left - tabs.getBoundingClientRect().left + tabs.scrollLeft;
    if (left < tabs.scrollLeft) tabs.scrollLeft = Math.max(0,left - 12);
    else if (left + selected.offsetWidth > tabs.scrollLeft + tabs.clientWidth) tabs.scrollLeft = left + selected.offsetWidth - tabs.clientWidth + 12;
  }
  function updateViewport() {
    viewportFrame = undefined;
    const root = document.documentElement;
    if (!mobileLayout.matches) {
      root.style.removeProperty('--mobile-navigation-lift');
      root.style.removeProperty('--mobile-navigation-height');
      root.style.removeProperty('--mobile-viewport-height');
      root.style.removeProperty('--mobile-viewport-top');
      return;
    }
    const viewport = window.visualViewport;
    const normalScale = viewport && Math.abs(viewport.scale - 1) < .01 && viewport.height > 0;
    // The keyboard can shrink only the visible viewport while fixed elements stay below it.
    const lift = normalScale
      ? Math.max(0,Math.round(root.clientHeight - viewport.height - Math.max(0,viewport.offsetTop))) : 0;
    const sizes = {
      '--mobile-navigation-lift':lift,
      '--mobile-navigation-height':Math.ceil(primary.getBoundingClientRect().height),
      '--mobile-viewport-height':normalScale ? viewport.height : root.clientHeight,
      '--mobile-viewport-top':normalScale ? Math.max(0,viewport.offsetTop) : 0
    };
    for (const [name,value] of Object.entries(sizes)) {
      if (root.style.getPropertyValue(name) !== `${value}px`) root.style.setProperty(name,`${value}px`);
    }
  }
  function scheduleViewport() {
    if (viewportFrame === undefined) viewportFrame = requestAnimationFrame(updateViewport);
  }
  function placeNavigation() {
    const destination = mobileLayout.matches ? mobileHost : navigationSlot.parentElement;
    if (primary.parentElement !== destination) {
      const focused = primary.contains(document.activeElement) ? document.activeElement : null;
      if (mobileLayout.matches) mobileHost.append(primary); else navigationSlot.after(primary);
      focused?.focus({preventScroll:true});
    }
    scheduleViewport();
    revealCurrentPage();
  }
  function go(route) {
    if (location.hash.slice(1) === route) OpenU60Motion.changePage(() => apply(),{animate:false}); else location.hash = route;
  }
  document.querySelector('.skip').addEventListener('click',event => {
    event.preventDefault();
    const main = document.querySelector('#main');
    main.tabIndex = -1; main.focus({preventScroll:true}); scrollTo({top:0,behavior:'instant'});
  });
  window.addEventListener('hashchange',() => {
    OpenU60Motion.changePage(() => apply(),{animate:resolve(location.hash.slice(1)).key !== current.key});
  });
  mobileLayout.addEventListener('change',placeNavigation);
  desktopLayout.addEventListener('change',()=>{ delete root.dataset.navigationMotion; placeNavigation(); });
  window.addEventListener('resize',() => { placeNavigation(); scheduleViewport(); },{passive:true});
  window.addEventListener('pageshow',placeNavigation);
  window.visualViewport?.addEventListener('resize',scheduleViewport,{passive:true});
  window.visualViewport?.addEventListener('scroll',scheduleViewport,{passive:true});
  if (typeof ResizeObserver === 'function') new ResizeObserver(scheduleViewport).observe(primary);
  globalThis.OpenU60Navigation = {go,get current(){return {group:current.group.id,page:current.page.id,key:current.key};}};
  apply(true);
  placeNavigation();
})();
