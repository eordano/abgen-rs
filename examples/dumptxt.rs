use abgen::unity::bundle_file::Bundle;
use std::path::PathBuf;
fn main(){
  for p in std::env::args().skip(1){
    let b=Bundle::load(&PathBuf::from(&p)).expect("load");
    let sf=b.serialized().expect("sf");
    for o in &sf.objects {
      if o.class_id!=49 {continue;}
      let v=sf.read_typetree(o).expect("tt");
      let m=v.as_map().unwrap();
      let name=m.get("m_Name").and_then(|x|x.as_str()).unwrap_or("");
      let script=m.get("m_Script");
      println!("=== {} ===",p);
      println!("name={}",name);
      if let Some(s)=script{ if let Some(st)=s.as_str(){ println!("script={:?}",st);} else {println!("script(non-str)={:?}",s);} }
    }
  }
}
