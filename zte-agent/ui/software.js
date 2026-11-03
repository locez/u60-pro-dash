'use strict';
(() => {
  const tr = globalThis.OpenU60I18n?.t || ((s,...v)=>s.replace(/\{(\d+)\}/g,(m,i)=>Number(i)<v.length?String(v[i]):m));
  const UI=OpenU60UI, API=OpenU60Console, {el,button}=UI;
  const versions=value=>String(value||'').match(/^v?(\d+)\.(\d+)\.(\d+)/)?.slice(1).map(Number);
  function compare(a,b){const x=versions(a),y=versions(b);if(!x||!y)return null;for(let i=0;i<3;i++)if(x[i]!==y[i])return x[i]<y[i]?-1:1;return 0;}
  function base64(bytes){let text='';for(let n=0;n<bytes.length;n+=8192)text+=String.fromCharCode(...bytes.subarray(n,n+8192));return btoa(text);}
  globalThis.OpenU60Installer=function(view,parent,{service,title,onInstalled=()=>{}}){
    const endpoint=`/api/software/${service}`, route=view.route;
    const panel=el('section','software-installer');panel.id=`${service}-installer`;
    const heading=el('h3','form-section-title',tr('Installation & updates'));
    const description=el('p','panel-note',tr('Automatically selects the official stable Linux ARM64 package for this router and verifies its SHA-256.'));
    const facts=el('dl','settings-facts'),actions=el('div','form-actions diagnostic-actions'),feedback=UI.status(panel),progress=el('progress','software-progress');progress.hidden=true;
    const check=button(tr('Check for updates'),checkRelease),install=button(tr('Install release'),installRelease,'primary');check.id=`${service}-check-update`;install.id=`${service}-install-release`;actions.append(check,install);
    const details=el('details','software-details'),summary=el('summary','',tr('Package details')),metadata=el('dl','settings-facts');details.append(summary,metadata);
    const manual=el('details','software-manual'),manualSummary=el('summary','',tr('Manual installation'));
    const fileForm=el('form','settings-form service-form');fileForm.id=`${service}-package-upload`;
    fileForm.append(el('p','panel-note',tr('Choose a Linux ARM64 package from your computer. Official filenames include the version; renamed packages need the version below.')));
    const label=el('label','field'),file=el('input');file.type='file';file.id=`${service}-package-file`;file.accept=service==='tailscale'?'.tgz':'.gz,.bin,application/octet-stream,application/x-executable';label.append(el('span','field-label',tr('Installation package')),file);
    const versionLabel=el('label','field'),version=el('input');version.type='text';version.id=`${service}-package-version`;version.placeholder=tr('Optional for official filenames');version.autocomplete='off';version.pattern='v?[0-9]+\\.[0-9]+\\.[0-9]+';versionLabel.append(el('span','field-label',tr('Package version')),version);
    const upload=button(tr('Upload and install'),uploadPackage,'primary');upload.id=`${service}-upload-install`;
    fileForm.append(label,versionLabel,el('p','field-help',service==='tailscale'?tr('Use the official tailscale_<version>_arm64.tgz archive. Maximum 120 MiB.'):tr('Use the Linux ARM64 .gz package or static executable. Maximum 96 MiB.')),upload);
    fileForm.addEventListener('submit',e=>e.preventDefault());manual.append(manualSummary,fileForm);
    panel.replaceChildren(heading,description,facts,actions,progress,feedback,details,manual);parent.append(panel);
    let generation=0, readEpoch=0, state=null, installed=null, loading=false, checking=false, loaded=false, uploading=false, uploadId=null, phase=null;
    const active=()=>OpenU60Navigation.current.key===route&&!API.locked&&API.connected;
    const current=epoch=>epoch===generation&&active();
    const currentVersion=()=>installed?.client_version||installed?.version||installed?.pinned_version||'';
    const fact=(host,name,value)=>{const row=el('div');row.append(el('dt','',name),el('dd','',value));host.append(row);};
    function sync(){
      const available=state?.available,order=compare(currentVersion(),available?.version),busy=!!state?.busy||uploading||!!installed?.updating;
      facts.replaceChildren();fact(facts,tr('Installed version'),!installed?tr('Waiting for status'):installed.installed?(currentVersion()||tr('Not reported')):tr('Not installed'));
      fact(facts,tr('Latest stable version'),available?.version||tr('Not checked'));
      fact(facts,tr('Platform'),'Linux ARM64');
      if(available){fact(facts,tr('Update status'),!installed?.installed?tr('Ready to install'):order===0?tr('Up to date'):order===1?tr('Installed version is newer than the stable release'):tr('Update available'));}
      check.textContent=checking?tr('Checking for updates…'):tr('Check for updates');check.disabled=!active()||busy||checking||UI.saving;
      install.textContent=installed?.installed?tr('Upgrade'):tr('Install release');
      install.disabled=!active()||busy||checking||UI.saving||!available||(installed?.installed&&order!==null&&order>=0);
      upload.disabled=!active()||busy||checking||UI.saving||!file.files.length;file.disabled=busy||UI.saving;version.disabled=busy||UI.saving;
      details.hidden=!available&&!state?.sha256;metadata.replaceChildren();
      if(available){fact(metadata,tr('Filename'),available.name);fact(metadata,'SHA-256',available.sha256);}
      else if(state?.sha256){fact(metadata,tr('Package version'),state.version||'—');fact(metadata,'SHA-256',state.sha256);}
      progress.hidden=!busy;const total=Number(state?.bytes_total)||file.files[0]?.size;
      if(total){progress.max=total;progress.value=state?.bytes_received||0;}else progress.removeAttribute('value');
    }
    function draw(value){
      const previous=phase;state=value;phase=value.phase;loaded=true;
      const messages={idle:'',uploading:tr('Uploading package…'),preparing:tr('Preparing installation…'),downloading:tr('Downloading official package…'),verifying:tr('Verifying package…'),installing:tr('Installing and checking the service…'),succeeded:tr('Installation complete. Current service state has been refreshed.'),failed:value.error||tr('Installation failed. Check the service state and try again.')};
      UI.say(feedback,messages[value.phase]||'',value.phase==='failed');sync();
      if(previous!==phase&&phase==='succeeded'){view.load();onInstalled();}
    }
    async function refresh(){
      if(loading||checking||uploading||UI.saving||!active())return;const epoch=generation,read=++readEpoch;loading=true;
      try{const value=await UI.request(`${endpoint}/status`);if(current(epoch)&&read===readEpoch)draw(value);}
      catch(error){if(current(epoch)&&read===readEpoch)UI.say(feedback,error.message,true);}
      finally{if(epoch===generation)loading=false;}
    }
    async function checkRelease(){
      if(!active()||checking||state?.busy)return;const epoch=generation,read=++readEpoch;checking=true;sync();UI.say(feedback,tr('Checking the official release source…'));
      try{const value=await UI.request(`${endpoint}/release`,{timeoutMs:90000});if(!current(epoch)||read!==readEpoch)return;draw(value);UI.say(feedback,tr('Official release information refreshed.'));}
      catch(error){if(current(epoch)&&read===readEpoch){if(state)state.available=null;UI.say(feedback,error.message,true);}}
      finally{if(epoch===generation){checking=false;sync();}}
    }
    async function installRelease(){
      const epoch=generation,release=state?.available;if(!active()||!release||state.busy)return;
      if(!await UI.confirm(title,tr('Install version {0}? The running service may restart briefly. Existing configuration is retained.',release.version),{label:tr('Install / upgrade')}))return;
      if(!current(epoch))return;
      readEpoch++;
      await UI.mutate(feedback,async live=>{
        try{
          const result=await UI.request(`${endpoint}/install`,UI.json('POST',{version:release.version,sha256:release.sha256}));
          if(live()&&current(epoch))draw(result);
        }catch(error){if(live()&&current(epoch))await reconcile(epoch);throw error;}
      });sync();
    }
    async function reconcile(epoch){
      try{
        const actual=await UI.request(`${endpoint}/status`);
        if(!current(epoch))return null;
        state=actual;phase=actual.phase;loaded=true;
        if(actual.phase==='succeeded'){view.load();onInstalled();}
        return actual;
      }catch{if(current(epoch))state={...state,busy:true};return null;}
    }
    async function cancelUpload(id){if(id&&!API.locked){try{await UI.request(`${endpoint}/upload/cancel`,UI.json('POST',{upload_id:id}));}catch{}}}
    async function uploadPackage(){
      const selected=file.files[0],epoch=generation;if(!active()||!selected||state?.busy||!fileForm.reportValidity())return;
      const limit=(service==='tailscale'?120:96)*1024*1024;
      if(!selected.size||selected.size>limit){UI.say(feedback,tr('This package is empty or exceeds the upload limit.'),true);return;}
      if(!await UI.confirm(title,tr('Upload and install {0}? The router will validate the package before replacing the installed software.',selected.name),{label:tr('Upload and install')}))return;
      if(!current(epoch))return;
      readEpoch++;uploading=true;sync();
      await UI.mutate(feedback,async live=>{
        let id=null;
        try{
          const start=await UI.request(`${endpoint}/upload`,UI.json('POST',{name:selected.name,size:selected.size,version:version.value.trim()}));id=start.upload_id;
          if(!live()||!current(epoch)){await cancelUpload(id);return;}uploadId=id;
          const chunkSize=Math.min(Number(start.chunk_size)||262144,262144);
          for(let offset=0;offset<selected.size;offset+=chunkSize){
            if(!live()||!current(epoch)){await cancelUpload(id);return;}
            const bytes=new Uint8Array(await selected.slice(offset,offset+chunkSize).arrayBuffer());
            if(!live()||!current(epoch)){await cancelUpload(id);return;}
            const result=await UI.request(`${endpoint}/upload/chunk`,UI.json('POST',{upload_id:id,offset,data:base64(bytes)}));
            if(current(epoch)){state={...state,busy:true,phase:'uploading',bytes_total:selected.size,bytes_received:result.bytes_received};UI.say(feedback,tr('Uploading package…'));sync();}
          }
          if(!live()||!current(epoch)){await cancelUpload(id);return;}
          const result=await UI.request(`${endpoint}/upload/install`,UI.json('POST',{upload_id:id}));uploadId=null;
          if(live()&&current(epoch)){fileForm.reset();delete fileForm.dataset.dirty;draw(result);}
        }catch(error){
          await cancelUpload(id);
          if(current(epoch)){
            uploadId=null;
            // A lost response may follow an accepted installation. Read its state;
            // never repeat the write or assume that cancelling an upload stops it.
            const actual=await reconcile(epoch);
            if(current(epoch)&&actual&&((actual.busy&&actual.phase!=='uploading')||actual.phase==='succeeded')){fileForm.reset();delete fileForm.dataset.dirty;}
          }
          throw error;
        }
      });
      if(current(epoch)){uploading=false;sync();}
    }
    fileForm.addEventListener('input',()=>{fileForm.dataset.dirty='true';sync();});file.addEventListener('change',sync);
    function clear(){const id=uploadId;generation++;state=null;installed=null;phase=null;loaded=false;loading=false;checking=false;uploading=false;uploadId=null;fileForm.reset();delete fileForm.dataset.dirty;manual.open=false;details.open=false;UI.say(feedback,'');sync();cancelUpload(id);}
    window.addEventListener('dashboard:navigate',()=>{if(OpenU60Navigation.current.key!==route)clear();});
    window.addEventListener('dashboard:connection',sync);window.addEventListener('dashboard:mutation-complete',sync);
    UI.track({route,load:refresh,refresh,clear,get loaded(){return loaded;},get dirty(){return !!file.files.length||!!version.value;}});
    setInterval(()=>{if(active()&&!document.hidden&&state?.busy&&!uploading)refresh();},2000);
    sync();return {drawInstalled(value){installed=value;sync();},clear};
  };
  globalThis.OpenU60DatabaseUpload=function(view,parent){
    const names={'geoip.dat':'geoip','geosite.dat':'geosite','geoip.metadb':'mmdb','geoip.db':'mmdb','country.mmdb':'mmdb','asn.mmdb':'asn','geolite2-asn.mmdb':'asn'};
    const panel=el('details','software-manual');panel.id='proxy-geodata-upload';
    panel.append(el('summary','',tr('Offline rule databases')),el('p','panel-note',tr('If the router cannot reach the download source, download the databases on your computer and upload them here. Stop this service before importing. Each file can be up to 64 MiB.')));
    const label=el('label','field'),file=el('input');file.id='proxy-geodata-file';file.type='file';file.accept='.dat,.mmdb,.metadb,.db';label.append(el('span','field-label',tr('Database file')),file);
    const upload=button(tr('Upload database'),uploadFile);upload.id='proxy-geodata-install';
    const progress=el('progress','software-progress');progress.hidden=true;const feedback=UI.status(panel);feedback.id='proxy-geodata-feedback';
    panel.append(label,el('p','field-help','GeoIP.dat · GeoSite.dat · geoip.metadb / Country.mmdb · ASN.mmdb'),upload,progress,feedback);parent.append(panel);
    let generation=0,uploading=false,pending=null,states={},loading=false,loaded=false;
    const active=()=>OpenU60Navigation.current.key===view.route&&!API.locked&&API.connected;
    const current=epoch=>epoch===generation&&active();
    const busy=()=>uploading||Object.values(states).some(s=>s.busy);
    function sync(){upload.disabled=!active()||UI.saving||busy()||!view.data?.installed||view.data?.running!==false||!file.files.length;file.disabled=UI.saving||busy();progress.hidden=!busy();}
    function report(){
      const rows=Object.entries(states).filter(([,s])=>s.phase!=='idle');
      const text=rows.map(([kind,s])=>`${({geoip:'GeoIP.dat',geosite:'GeoSite.dat',mmdb:'geoip.metadb',asn:'ASN.mmdb'})[kind]}: ${s.phase==='succeeded'?tr('Database imported. You can now validate and save the configuration.'):s.phase==='failed'?s.error||tr('Import failed.'):tr('Uploading and checking database…')}`).join('\n');
      UI.say(feedback,text,rows.some(([,s])=>s.phase==='failed'));sync();
    }
    async function refresh(){
      if(!active()||loading||uploading||UI.saving)return;const epoch=generation;loading=true;
      try{const result=await Promise.all(['geoip','geosite','mmdb','asn'].map(async kind=>[kind,await UI.request(`/api/software/mihomo-${kind}/status`)]));if(current(epoch)){states=Object.fromEntries(result);loaded=true;report();}}
      catch(error){if(current(epoch))UI.say(feedback,error.message,true);}
      finally{if(current(epoch))loading=false;}
    }
    async function cancel(job){if(job&&!API.locked){try{await UI.request(`${job.endpoint}/upload/cancel`,UI.json('POST',{upload_id:job.id}));}catch{}}}
    async function uploadFile(){
      const selected=file.files[0],epoch=generation;if(!active()||busy()||UI.saving||view.data?.running!==false||!selected)return;
      const kind=names[selected.name.toLowerCase()];
      if(!kind){UI.say(feedback,tr('Choose GeoIP.dat, GeoSite.dat, geoip.metadb, Country.mmdb or ASN.mmdb.'),true);return;}
      if(!selected.size||selected.size>64*1024*1024){UI.say(feedback,tr('The database is empty or exceeds 64 MiB.'),true);return;}
      if(!await UI.confirm(tr('Import rule database'),tr('Validate and replace {0}? The service stays stopped and your configuration draft is retained.',selected.name),{label:tr('Upload database')}))return;
      if(!current(epoch))return;
      uploading=true;sync();const endpoint=`/api/software/mihomo-${kind}`;
      await UI.mutate(feedback,async live=>{
        let job=null;
        try{
          const start=await UI.request(`${endpoint}/upload`,UI.json('POST',{name:selected.name,size:selected.size}));job={endpoint,id:start.upload_id};
          if(!live()||!current(epoch)){await cancel(job);return;}pending=job;
          const chunkSize=Math.min(Number(start.chunk_size)||262144,262144);
          progress.max=selected.size;progress.value=0;
          for(let offset=0;offset<selected.size;offset+=chunkSize){
            const bytes=new Uint8Array(await selected.slice(offset,offset+chunkSize).arrayBuffer());
            if(!live()||!current(epoch)){await cancel(job);return;}
            const result=await UI.request(`${endpoint}/upload/chunk`,UI.json('POST',{upload_id:job.id,offset,data:base64(bytes)}));
            if(live()&&current(epoch)){progress.value=result.bytes_received;UI.say(feedback,tr('Uploading and checking database…'));}
          }
          if(!live()||!current(epoch)){await cancel(job);return;}
          const result=await UI.request(`${endpoint}/upload/install`,UI.json('POST',{upload_id:job.id}));
          if(live()&&current(epoch)){pending=null;file.value='';states[kind]=result;report();}
        }catch(error){
          await cancel(job);
          // Read back a possibly accepted job; never retry the install write.
          if(current(epoch)){
            try{const actual=await UI.request(`${endpoint}/status`);if(current(epoch))states[kind]=actual;}catch{if(current(epoch))states[kind]={busy:true,phase:'preparing'};}
            if(!current(epoch))return;
            pending=null;if(states[kind].phase==='succeeded'||(states[kind].busy&&states[kind].phase!=='uploading'))file.value='';
            UI.say(feedback,error.message,true);
          }
        }
      });
      if(current(epoch)){uploading=false;sync();}
    }
    function clear(){const job=pending;generation++;pending=null;uploading=false;loading=false;loaded=false;states={};file.value='';panel.open=false;UI.say(feedback,'');sync();cancel(job);}
    file.addEventListener('change',sync);
    window.addEventListener('dashboard:navigate',()=>{if(OpenU60Navigation.current.key!==view.route)clear();});
    window.addEventListener('dashboard:connection',sync);window.addEventListener('dashboard:mutation-complete',sync);
    UI.track({route:view.route,load:refresh,refresh,clear,get loaded(){return loaded;},get dirty(){return !!file.files.length;}});
    setInterval(()=>{if(active()&&!document.hidden&&busy()&&!uploading)refresh();},2000);
    return {clear,sync};
  };
})();
