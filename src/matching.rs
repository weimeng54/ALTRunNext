pub fn prepare_legacy(pattern:&str)->String{
    let mut output=String::new();let mut escaped=false;let mut class=false;let mut previous='\0';
    for c in pattern.chars(){
        if escaped{output.push(c);escaped=false;previous=c;continue;}
        if c=='\\'{output.push(c);escaped=true;continue;}
        if c=='['{class=true;}if c==']'{class=false;}
        if !class&&c=='*'&&previous!='.'{output.push_str(".*");}
        else if !class&&c=='?'&&previous!='(' {output.push('.');}
        else{output.push(c);}previous=c;
    }output
}
pub fn advanced(pattern:&str)->bool{
    pattern.contains("(?")||pattern.contains("\\k")||pattern.contains("\\g")||pattern.contains("\\p")||pattern.as_bytes().windows(2).any(|p|p[0]==b'\\'&&p[1].is_ascii_digit())
}
pub fn positions(pattern:&str,aliases:&[String],prefix:bool)->Result<Vec<Option<usize>>,String>{
    if pattern.chars().count()>512{return Err("正则表达式过长".into());}
    if advanced(pattern){
        let regex=fancy_regex::RegexBuilder::new(pattern).case_insensitive(true).backtrack_limit(100_000).build().map_err(|e|e.to_string())?;
        aliases.iter().map(|alias|regex.find(alias).map_err(|e|e.to_string()).map(|found|found.and_then(|m|if prefix&&m.start()!=0{None}else{Some(alias[..m.start()].chars().count())}))).collect()
    }else{
        let regex=regex_lite::RegexBuilder::new(pattern).case_insensitive(true).size_limit(512*1024).build().map_err(|e|e.to_string())?;
        Ok(aliases.iter().map(|alias|regex.find(alias).and_then(|m|if prefix&&m.start()!=0{None}else{Some(alias[..m.start()].chars().count())})).collect())
    }
}
