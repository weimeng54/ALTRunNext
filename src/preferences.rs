use std::collections::BTreeMap;
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct Preferences{pub values:BTreeMap<String,String>}
impl Default for Preferences{fn default()->Self{Self{values:BTreeMap::new()}}}
impl Preferences{
    pub fn parse(_text:&str)->Result<Self,String>{Err("not implemented".into())}
    pub fn text(&self)->String{String::new()}
    pub fn get(&self,key:&str)->String{self.values.get(key).cloned().unwrap_or_default()}
    pub fn flag(&self,key:&str)->bool{self.get(key)=="1"}
    pub fn set(&mut self,key:&str,value:&str)->Result<(),String>{self.values.insert(key.into(),value.into());Ok(())}
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct Hotkey{pub modifiers:u32,pub key:u32}
pub fn parse_hotkey(_value:&str)->Result<Option<Hotkey>,String>{Err("not implemented".into())}
