use abgen::unity::bundle_file::Bundle;
use std::env;
use std::path::PathBuf;
fn main(){
 for p in env::args().skip(1){
  let b=Bundle::load(&PathBuf::from(&p)).unwrap();
  let sf=b.serialized().unwrap();
  println!("=== {} platform={} ver={} objects={}",p, sf.target_platform, sf.version, sf.objects.len());
  for o in &sf.objects { println!("  class_id={} pid={} len={}", o.class_id,o.path_id,o.data.len()); }
 }
}
