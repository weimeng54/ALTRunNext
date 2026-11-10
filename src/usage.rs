use std::collections::BTreeMap;
#[derive(Clone,Debug,Default,PartialEq,Eq)]
pub struct Usage{pub counts:BTreeMap<String,u64>,pub recent:Vec<String>,pub favourites:BTreeMap<String,String>,pub parameters:BTreeMap<String,Vec<String>>}
impl Usage{
    pub fn parse(_text:&str)->Result<Self,String>{Err("not implemented".into())}
    pub fn text(&self)->String{String::new()}
    pub fn record(&mut self,_id:&str,_query:&str,_parameter:Option<&str>,_limit:usize){}
}
