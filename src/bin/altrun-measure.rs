//! A narrowly scoped local test instrument. No Enter key or command execution.
//! Desktop pixels are compared with a stable reference of each target's own
//! results control. Report intervals/upper bounds, never claim hardware present
//! timestamps, physical keyboard latency, IME coverage, or a complete gate pass.
#[cfg(windows)]
mod windows {
    use std::{ffi::c_void, fmt::Write, fs, mem::{size_of,zeroed}, ptr::{null,null_mut}, thread, time::{Duration,Instant}};
    use windows_sys::Win32::{Foundation::*, Graphics::Gdi::*, System::LibraryLoader::GetModuleHandleW, UI::{HiDpi::*,Input::KeyboardAndMouse::*,WindowsAndMessaging::*}};

    fn wide(s:&str)->Vec<u16>{s.encode_utf16().chain(Some(0)).collect()}
    fn class(hwnd:HWND)->String{unsafe{let mut b=[0_u16;128];let n=GetClassNameW(hwnd,b.as_mut_ptr(),128);String::from_utf16_lossy(&b[..n as usize])}}
    unsafe extern "system" fn enumerate(hwnd:HWND,param:LPARAM)->i32{unsafe{(&mut *(param as *mut Vec<HWND>)).push(hwnd);}1}
    fn find_target(pid:u32,legacy:bool)->Result<HWND,String>{
        let mut all=Vec::<HWND>::new();unsafe{EnumWindows(Some(enumerate),&mut all as *mut _ as isize);}
        let matches:Vec<_>=all.into_iter().filter(|&h|{let mut p=0;unsafe{GetWindowThreadProcessId(h,&mut p);}p==pid&&class(h)==if legacy{"TALTRunForm"}else{"ALTRun.Native.PerformanceProbe"}}).collect();
        if matches.len()!=1{return Err("Expected one verified ALTRun target window".into());}Ok(matches[0])
    }
    fn children(hwnd:HWND)->Result<(HWND,HWND),String>{
        let mut all=Vec::<HWND>::new();unsafe{EnumChildWindows(hwnd,Some(enumerate),&mut all as *mut _ as isize);}
        let edit=all.iter().copied().find(|&h|{let s=unsafe{GetWindowLongPtrW(h,GWL_STYLE)}as u32;class(h).to_lowercase().ends_with("edit")&&s&ES_READONLY as u32==0&&s&WS_DISABLED==0&&s&WS_VISIBLE!=0});
        let list=all.iter().copied().find(|&h|class(h).to_lowercase().ends_with("listbox"));
        edit.zip(list).ok_or("Cannot identify native editable query/list controls".into())
    }
    fn set_query(edit:HWND,q:&str)->Result<(),String>{unsafe{let b=wide(q);let mut result=0;if SendMessageTimeoutW(edit,WM_SETTEXT,0,b.as_ptr() as isize,SMTO_ABORTIFHUNG,2000,&mut result)==0{return Err("Query message timed out".into());}}Ok(())}
    fn message(hwnd:HWND,msg:u32,w:usize,l:isize)->Result<usize,String>{
        let mut result=0;
        if unsafe{SendMessageTimeoutW(hwnd,msg,w,l,SMTO_ABORTIFHUNG|SMTO_ERRORONEXIT,2000,&mut result)}==0{return Err("Target message failed or timed out".into());}Ok(result)
    }
    fn get_query(edit:HWND)->Result<String,String>{
        let length=message(edit,WM_GETTEXTLENGTH,0,0)?;
        if length>32768{return Err("Original query too long to safely preserve".into());}
        let mut b=vec![0_u16;length+1];let n=message(edit,WM_GETTEXT,b.len(),b.as_mut_ptr() as isize)?;
        if n>length{return Err("Query changed while preserving it".into());}Ok(String::from_utf16_lossy(&b[..n]))
    }
    fn foreground_pid()->u32{unsafe{let mut p=0;GetWindowThreadProcessId(GetForegroundWindow(),&mut p);p}}
    fn input(keys:&[(u16,bool)])->Result<(),String>{
        let values:Vec<INPUT>=keys.iter().map(|&(key,up)|{let mut i:INPUT=unsafe{zeroed()};i.r#type=INPUT_KEYBOARD;i.Anonymous.ki=KEYBDINPUT{wVk:key,wScan:0,dwFlags:if up{KEYEVENTF_KEYUP}else{0},time:0,dwExtraInfo:0};i}).collect();
        if unsafe{SendInput(values.len() as u32,values.as_ptr(),size_of::<INPUT>() as i32)}!=values.len() as u32{return Err("SendInput failed; no fallback activation was used".into());}Ok(())
    }
    fn wake(legacy:bool)->Result<(),String>{
        if [VK_CONTROL,VK_SHIFT,VK_MENU].iter().any(|&k|unsafe{GetAsyncKeyState(k as i32)}<0){return Err("Release modifier keys before measurement".into());}
        if legacy{input(&[(VK_MENU,false),(0x52,false),(0x52,true),(VK_MENU,true)])}
        else{input(&[(VK_CONTROL,false),(VK_MENU,false),(0x52,false),(0x52,true),(VK_MENU,true),(VK_CONTROL,true)])}
    }
    fn wait_visible(hwnd:HWND,visible:bool)->Result<(),String>{let start=Instant::now();while (unsafe{IsWindowVisible(hwnd)}!=0)!=visible{if start.elapsed()>Duration::from_secs(2){return Err("Target did not reach requested visibility".into());}thread::sleep(Duration::from_millis(1));}Ok(())}


    struct Capture{screen:HDC,memory:HDC,bitmap:HBITMAP,old:HGDIOBJ,rect:RECT,pixels:Vec<u8>,info:BITMAPINFO}
    impl Capture{
        fn new(rect:RECT)->Result<Self,String>{unsafe{
            let width=rect.right-rect.left;let height=rect.bottom-rect.top;
            if width<=0||height<=0||width>4096||height>4096{return Err("Invalid results capture region".into());}
            let screen=GetDC(null_mut());let memory=CreateCompatibleDC(screen);let bitmap=CreateCompatibleBitmap(screen,width,height);
            if screen.is_null()||memory.is_null()||bitmap.is_null(){if !bitmap.is_null(){DeleteObject(bitmap);}if !memory.is_null(){DeleteDC(memory);}if !screen.is_null(){ReleaseDC(null_mut(),screen);}return Err("Cannot allocate capture buffers".into());}
            let old=SelectObject(memory,bitmap);let mut info:BITMAPINFO=zeroed();info.bmiHeader.biSize=size_of::<BITMAPINFOHEADER>()as u32;info.bmiHeader.biWidth=width;info.bmiHeader.biHeight=-height;info.bmiHeader.biPlanes=1;info.bmiHeader.biBitCount=32;info.bmiHeader.biCompression=BI_RGB;
            Ok(Self{screen,memory,bitmap,old,rect,pixels:vec![0;width as usize*height as usize*4],info})
        }}
        fn hash(&mut self)->Result<u64,String>{unsafe{
            let width=self.rect.right-self.rect.left;let height=self.rect.bottom-self.rect.top;
            if BitBlt(self.memory,0,0,width,height,self.screen,self.rect.left,self.rect.top,SRCCOPY|CAPTUREBLT)==0{return Err("Desktop pixel copy failed".into());}
            // GetDIBits requires the bitmap to be deselected from its DC.
            SelectObject(self.memory,self.old);
            let rows=GetDIBits(self.screen,self.bitmap,0,height as u32,self.pixels.as_mut_ptr()as *mut c_void,&mut self.info,DIB_RGB_COLORS);
            SelectObject(self.memory,self.bitmap);
            if rows!=height{return Err("Incomplete desktop pixel read".into());}
            Ok(self.pixels.iter().fold(0xcbf29ce484222325_u64,|h,&b|(h^b as u64).wrapping_mul(0x100000001b3)))
        }}
    }
    impl Drop for Capture{fn drop(&mut self){unsafe{SelectObject(self.memory,self.old);DeleteObject(self.bitmap);DeleteDC(self.memory);ReleaseDC(null_mut(),self.screen);}}}
    const QUERIES:[&str;5]=["","a","e","o","r"];
    fn rectangle(hwnd:HWND)->Result<RECT,String>{let mut r:RECT=unsafe{zeroed()};if unsafe{GetWindowRect(hwnd,&mut r)}==0{return Err("Cannot read results geometry".into());}Ok(r)}
    fn same_rect(a:RECT,b:RECT)->bool{a.left==b.left&&a.top==b.top&&a.right==b.right&&a.bottom==b.bottom}
    struct Reference{capture:Capture,hash:u64,list_rect:RECT}
    struct Target{pid:u32,hwnd:HWND,edit:HWND,list:HWND,legacy:bool,saved_query:String,references:Vec<Reference>}
    impl Target{
        fn new(pid:u32,legacy:bool)->Result<Self,String>{
            let hwnd=find_target(pid,legacy)?;let(edit,list)=children(hwnd)?;
            if unsafe{IsWindowVisible(hwnd)}!=0{return Err("Start with both launchers hidden; no visible input is altered".into());}
            Ok(Self{pid,hwnd,edit,list,legacy,saved_query:get_query(edit)?,references:Vec::new()})
        }
        fn verify(&self)->Result<(),String>{
            let expected=if self.legacy{"TALTRunForm"}else{"ALTRun.Native.PerformanceProbe"};
            let mut pid=0;unsafe{GetWindowThreadProcessId(self.hwnd,&mut pid);}
            if pid!=self.pid||class(self.hwnd)!=expected{return Err("Verified target disappeared or changed".into());}
            for child in [self.edit,self.list]{let mut p=0;unsafe{GetWindowThreadProcessId(child,&mut p);}if p!=self.pid||unsafe{IsChild(self.hwnd,child)}==0{return Err("Verified input/results control changed".into());}}
            Ok(())
        }
        fn active(&self)->Result<(),String>{self.verify()?;if unsafe{IsWindowVisible(self.hwnd)}==0||unsafe{GetForegroundWindow()}!=self.hwnd||foreground_pid()!=self.pid{return Err("Target hidden or foreground changed; observation rejected".into());}Ok(())}
        fn query(&self,q:&str)->Result<(),String>{self.verify()?;set_query(self.edit,q)}
        fn hide(&self)->Result<(),String>{
            self.verify()?;self.query("")?;
            if unsafe{IsWindowVisible(self.hwnd)}==0{return Ok(());}
            // Direct, bounded messages cannot deliver Escape to another app.
            message(self.edit,WM_KEYDOWN,VK_ESCAPE as usize,1)?;
            message(self.edit,WM_KEYUP,VK_ESCAPE as usize,0xc0000001_u32 as isize)?;
            wait_visible(self.hwnd,false)
        }
        fn wake(&self)->Result<(),String>{self.verify()?;wake(self.legacy)?;wait_visible(self.hwnd,true)?;self.active()}
        fn prepare(&mut self)->Result<(),String>{
            self.wake()?;thread::sleep(Duration::from_millis(100));self.active()?;
            let mut region=rectangle(self.list)?;
            // One fixed union rectangle per target includes all list resizing.
            for query in QUERIES.iter().skip(1){self.query(query)?;thread::sleep(Duration::from_millis(100));self.active()?;let r=rectangle(self.list)?;region.left=region.left.min(r.left);region.top=region.top.min(r.top);region.right=region.right.max(r.right);region.bottom=region.bottom.max(r.bottom);}
            self.hide()?;
            for (i,query) in QUERIES.iter().enumerate(){
                if i==0{self.wake()?;}else{self.query(query)?;}
                thread::sleep(Duration::from_millis(100));self.active()?;
                if message(self.list,LB_GETCOUNT,0,0)?<2{return Err("Every fixed query must have at least two results in both targets".into());}
                let list_rect=rectangle(self.list)?;let mut capture=Capture::new(region)?;
                self.active()?;let hash=capture.hash()?;self.active()?;
                thread::sleep(Duration::from_millis(20));
                if !same_rect(rectangle(self.list)?,list_rect)||capture.hash()?!=hash{return Err("Unstable reference pixels or geometry".into());}self.active()?;
                self.references.push(Reference{capture,hash,list_rect});
            }
            if self.references.windows(2).any(|w|w[0].hash==w[1].hash){return Err("Fixed query transitions need distinct pixels in both targets".into());}
            self.hide()
        }
        fn pre_transition(&mut self,index:usize)->Result<(),String>{
            self.verify()?;
            if self.references[index].capture.hash()?==self.references[index].hash{return Err("Expected pixels already visible before action; transition rejected".into());}Ok(())
        }
        fn observe(&mut self,index:usize,start:Instant)->Result<(u128,u128),String>{
            let mut lower=0;
            loop{
                let begin=start.elapsed().as_nanos();self.verify()?;
                if unsafe{IsWindowVisible(self.hwnd)}!=0&&unsafe{GetForegroundWindow()}==self.hwnd{
                    let geometry=rectangle(self.list)?;let hash=self.references[index].capture.hash()?;
                    self.active()?;
                    if same_rect(geometry,self.references[index].list_rect)&&same_rect(rectangle(self.list)?,geometry)&&hash==self.references[index].hash{return Ok((lower,start.elapsed().as_nanos()));}
                }
                lower=begin;
                if start.elapsed()>Duration::from_secs(2){return Err("Expected result pixels not observed; no latency sample accepted".into());}
                thread::sleep(Duration::from_micros(100));
            }
        }
    }
    impl Drop for Target{fn drop(&mut self){
        if self.verify().is_ok(){let _=self.hide();let _=self.query(&self.saved_query);}
    }}

    pub fn run()->Result<(),String>{
        let args:Vec<_>=std::env::args().collect();
        let value=|name:&str|args.iter().position(|a|a==name).and_then(|i|args.get(i+1)).cloned().ok_or(format!("Missing {name}"));
        if !args.iter().any(|a|a=="--confirm-original-auto-execute-off"){return Err("First verify original ExecuteIfOnlyOne=0; measurement never sends Enter".into());}
        let original:u32=value("--original-pid")?.parse().map_err(|_|"Invalid original PID")?;
        let probe:u32=value("--probe-pid")?.parse().map_err(|_|"Invalid probe PID")?;
        let output=std::path::PathBuf::from(value("--output")?);
        if !output.is_absolute()||!output.to_string_lossy().to_ascii_lowercase().starts_with("d:\\"){return Err("Output must stay on D:".into());}
        let rounds:usize=value("--rounds").unwrap_or("50".into()).parse().map_err(|_|"Invalid rounds")?;
        if !(1..=1000).contains(&rounds){return Err("Rounds must be between 1 and 1000".into());}
        unsafe{SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);}
        let mut targets=[Target::new(original,true)?,Target::new(probe,false)?];
        // Opaque, non-topmost backdrop avoids reference drift from unrelated
        // windows behind the original's translucent skin. Always removed.
        let mut root_rect:RECT=unsafe{zeroed()};unsafe{GetWindowRect(targets[0].hwnd,&mut root_rect);}
        let scene=unsafe{CreateWindowExW(WS_EX_TOOLWINDOW,wide("STATIC").as_ptr(),wide("").as_ptr(),WS_POPUP|WS_BORDER,root_rect.left-16,root_rect.top-16,1800,1200,null_mut(),null_mut(),GetModuleHandleW(null()),null())};
        if scene.is_null(){return Err("Cannot create stable measurement backdrop".into());}
        struct Scene(HWND);impl Drop for Scene{fn drop(&mut self){unsafe{DestroyWindow(self.0);}}}
        let _scene=Scene(scene);unsafe{ShowWindow(scene,SW_SHOWNOACTIVATE);UpdateWindow(scene);}
        // Prewarm without counting preparation as a measurement.
        for target in &mut targets{target.prepare()?;}
        let mut rows=String::new();let mut count=0;
        for iteration in 0..rounds{
            for offset in 0..2{
                let index=(iteration+offset)%2;let target=&mut targets[index];
                target.pre_transition(0)?;let start=Instant::now();target.verify()?;wake(target.legacy)?;let(lo,hi)=target.observe(0,start)?;
                if count>0{rows.push(',');}count+=1;
                write!(&mut rows,"{{\"target\":\"{}\",\"iteration\":{},\"metric\":\"hotkey_to_results_pixels\",\"lower_ns\":{},\"upper_ns\":{}}}",if index==0{"original"}else{"probe"},iteration,lo,hi).unwrap();
                for (reference,_) in QUERIES.iter().enumerate().skip(1){
                    let query=QUERIES[reference];target.active()?;target.pre_transition(reference)?;let start=Instant::now();target.query(query)?;let(lo,hi)=target.observe(reference,start)?;
                    rows.push(',');count+=1;
                    write!(&mut rows,"{{\"target\":\"{}\",\"iteration\":{},\"metric\":\"settext_to_results_pixels\",\"query_id\":{},\"lower_ns\":{},\"upper_ns\":{}}}",if index==0{"original"}else{"probe"},iteration,reference,lo,hi).unwrap();
                }
                target.hide()?;
            }
        }
        let report=format!("{{\"measurement_kind\":\"desktop-pixel-upper-bounds\",\"decision\":\"pending\",\"samples\":[{}],\"limitations\":[\"SendInput software event start, not physical key travel\",\"SetText is not physical typing or IME\",\"Pixel capture intervals, not hardware-present timestamps\",\"Warm samples only; idle, load, IME and 24h remain unverified\"]}}",rows);
        if let Some(parent)=output.parent(){fs::create_dir_all(parent).map_err(|_|"Cannot create output folder")?;}
        fs::write(output,report).map_err(|_|"Cannot save private measurement")?;
        println!("Captured {count} bounded pixel observations; performance gate remains pending.");Ok(())
    }
}

fn main(){
    #[cfg(windows)]
    if let Err(message)=windows::run(){eprintln!("Measurement not accepted: {message}");std::process::exit(2);}
}
