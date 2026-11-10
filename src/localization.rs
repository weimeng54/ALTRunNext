use std::{cell::RefCell,collections::BTreeMap,path::Path};
thread_local!{static LANGUAGE:RefCell<BTreeMap<String,String>>=const{RefCell::new(BTreeMap::new())};}
pub fn tr(value:&str)->String{LANGUAGE.with(|l|l.borrow().get(value).cloned().unwrap_or_else(||value.to_owned()))}
pub fn install(language:&str,base:&Path)->Result<(),String>{let mut map=BTreeMap::new();if language.eq_ignore_ascii_case("EN")||language.eq_ignore_ascii_case("English"){
    for line in include_str!("../assets/english.txt").lines(){if let Some((key,value))=line.split_once('='){map.insert(key.replace("\\t","\t"),value.replace("\\t","\t"));}}
}else if !matches!(language,""|"简体中文"|"ZH-CN"){let path=Path::new(language);let path=if path.is_absolute(){path.to_path_buf()}else{base.join(path)};let bytes=std::fs::read(path).map_err(|e|format!("无法读取语言文件：{e}"))?;if bytes.len()>1024*1024{return Err("语言文件过大".into());}for line in altrun_probe::library::decode(&bytes)?.lines(){let line=line.trim();if line.starts_with([';','#','[']){continue;}if let Some((key,value))=line.split_once('='){map.insert(key.trim().to_owned(),value.trim().to_owned());}}}LANGUAGE.with(|l|*l.borrow_mut()=map);Ok(())}
