use abgen::unity::bundle_file::{Bundle, FileContent};
use abgen::value::Value;
fn g<'a>(v: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut cur = v;
    for k in path {
        cur = cur.get(k)?;
    }
    Some(cur)
}
fn main() {
    let p = std::env::args().nth(1).unwrap();
    let b = Bundle::load(std::path::Path::new(&p)).unwrap();
    for f in &b.files {
        if let FileContent::Serialized(sf) = &f.content {
            for o in &sf.objects {
                if o.class_id != 74 {
                    continue;
                }
                let v = sf.read_typetree(o).unwrap();
                let name = v.get("m_Name").and_then(|x| x.as_str()).unwrap_or("");
                let alen = |p: &[&str]| {
                    g(&v, p)
                        .and_then(|x| x.as_array().map(|a| a.len()))
                        .unwrap_or(0)
                };
                let ival = |p: &[&str]| g(&v, p).and_then(|x| x.as_i64()).unwrap_or(-1);
                println!(
                    "== clip {} pid={} streamed: words={} curves={} | dense: frames={} curves={} begin={:?} | const: n={} | start={:?} stop={:?} rate={:?}",
                    name,
                    o.path_id,
                    alen(&["m_MuscleClip", "m_Clip", "data", "m_StreamedClip", "data"]),
                    ival(&["m_MuscleClip", "m_Clip", "data", "m_StreamedClip", "curveCount"]),
                    ival(&["m_MuscleClip", "m_Clip", "data", "m_DenseClip", "m_FrameCount"]),
                    ival(&["m_MuscleClip", "m_Clip", "data", "m_DenseClip", "m_CurveCount"]),
                    g(&v, &["m_MuscleClip", "m_Clip", "data", "m_DenseClip", "m_BeginTime"]),
                    alen(&["m_MuscleClip", "m_Clip", "data", "m_ConstantClip", "data"]),
                    g(&v, &["m_MuscleClip", "m_StartTime"]),
                    g(&v, &["m_MuscleClip", "m_StopTime"]),
                    g(&v, &["m_SampleRate"]),
                );
                if let Some(gb) =
                    g(&v, &["m_ClipBindingConstant", "genericBindings"]).and_then(|x| x.as_array())
                {
                    for (i, gbi) in gb.iter().enumerate() {
                        println!(
                            "  [{i:3}] path={:>10} attr={:>10} type={} custom={}",
                            gbi.get("path").and_then(|x| x.as_i64()).unwrap_or(-1),
                            gbi.get("attribute").and_then(|x| x.as_i64()).unwrap_or(-1),
                            gbi.get("typeID").and_then(|x| x.as_i64()).unwrap_or(-1),
                            gbi.get("customType").and_then(|x| x.as_i64()).unwrap_or(-1),
                        );
                    }
                }
            }
        }
    }
}
