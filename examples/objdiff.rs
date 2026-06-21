use abgen::unity::bundle_file::Bundle;
use std::path::PathBuf;
fn main(){
 let mut a=std::env::args().skip(1);
 let o=PathBuf::from(a.next().unwrap());let r=PathBuf::from(a.next().unwrap());
 let ob=Bundle::load(&o).unwrap();let rb=Bundle::load(&r).unwrap();
 let osf=ob.serialized().unwrap();let rsf=rb.serialized().unwrap();
 for oo in &osf.objects {
   if let Some(ro)=rsf.objects.iter().find(|x|x.path_id==oo.path_id){
     if oo.data==ro.data {continue;}
     let n=oo.data.len().min(ro.data.len());
     let mut first=None;let mut cnt=0;
     for i in 0..n { if oo.data[i]!=ro.data[i]{ if first.is_none(){first=Some(i);} cnt+=1;}}
     println!("class={} pid={} len o={} r={} firstdiff={:?} difbytes={}",oo.class_id,oo.path_id,oo.data.len(),ro.data.len(),first,cnt);
     if let Some(f)=first {
       let s=f.saturating_sub(4); let e=(f+16).min(n);
       println!("  ours[{}..{}]={:02x?}",s,e,&oo.data[s..e]);
       println!("  ref [{}..{}]={:02x?}",s,e,&ro.data[s..e]);
     }
   }
 }
}
