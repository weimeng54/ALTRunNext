#![cfg(windows)]
mod review_native_support;
use review_native_support::*;
#[test]
fn manager_test_respects_debug_and_explicit_safe_modes(){
    let root=std::env::current_dir().unwrap().join("target/review-execution-policy");std::fs::create_dir_all(&root).unwrap();
    for(name,mode,allowed)in[("debug",None,false),("safe",Some("--safe-mode"),false),("enabled",Some("--enable-launch"),true)]{
        let marker=root.join(format!("{name}.txt"));let data=root.join(format!("{name}.tsv"));let _=std::fs::remove_file(&marker);std::fs::write(&data,format!("test\tSynthetic test\t\"{}\" /D /C echo ok>\"{}\"\t0\tnone\n",shell().display(),marker.display())).unwrap();
        let(process,launcher)=start(&data,mode);let dialog=manager(launcher,process.0.id());command(dialog,2137);
        if allowed{wait(||marker.exists());assert_eq!(std::fs::read_to_string(&marker).unwrap().trim(),"ok");}else{assert!(has_text(dialog,"预览"),"blocked test launch needs visible preview feedback");std::thread::sleep(std::time::Duration::from_millis(350));assert!(!marker.exists(),"safe/default Debug manager test executed a shortcut");}
    }
}
