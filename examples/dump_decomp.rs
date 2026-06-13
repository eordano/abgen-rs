use abgen::lz4;
use std::env;
use std::fs;
use std::io::{Cursor, Read};

fn read_cstring(cur: &mut Cursor<&Vec<u8>>) -> String {
    let mut out = Vec::new();
    let mut buf = [0u8; 1];
    loop {
        cur.read_exact(&mut buf).unwrap();
        if buf[0] == 0 {
            break;
        }
        out.push(buf[0]);
    }
    String::from_utf8(out).unwrap()
}

fn main() {
    let path = env::args().nth(1).expect("bundle path");
    let out_dir = env::args().nth(2).expect("out_dir");
    fs::create_dir_all(&out_dir).unwrap();
    let data = fs::read(&path).unwrap();
    let mut cur = Cursor::new(&data);

    let mut sig = [0u8; 8];
    cur.read_exact(&mut sig).unwrap();
    assert_eq!(&sig, b"UnityFS\0");

    let mut fmt = [0u8; 4];
    cur.read_exact(&mut fmt).unwrap();
    let _fmt = u32::from_be_bytes(fmt);

    let _ver_player = read_cstring(&mut cur);
    let _ver_engine = read_cstring(&mut cur);

    let mut buf8 = [0u8; 8];
    cur.read_exact(&mut buf8).unwrap();
    let _total = u64::from_be_bytes(buf8);

    let mut buf4 = [0u8; 4];
    cur.read_exact(&mut buf4).unwrap();
    let compressed_blockinfo = u32::from_be_bytes(buf4) as usize;
    cur.read_exact(&mut buf4).unwrap();
    let uncompressed_blockinfo = u32::from_be_bytes(buf4) as usize;
    cur.read_exact(&mut buf4).unwrap();
    let flags = u32::from_be_bytes(buf4);

    if _fmt >= 7 {
        let p = cur.position() as usize;
        let pad = (16 - (p % 16)) % 16;
        cur.set_position((p + pad) as u64);
    }
    let header_end = cur.position() as usize;
    let blockinfo_offset = if flags & 0x80 != 0 {
        data.len() - compressed_blockinfo
    } else {
        header_end
    };
    let blockinfo_compressed = &data[blockinfo_offset..blockinfo_offset + compressed_blockinfo];
    let comp_type = flags & 0x3f;
    let blockinfo_bytes: Vec<u8> = match comp_type {
        0 => blockinfo_compressed.to_vec(),
        2 | 3 => lz4::decompress(blockinfo_compressed, uncompressed_blockinfo).unwrap(),
        _ => panic!("unknown compression {}", comp_type),
    };

    let mut bi = Cursor::new(&blockinfo_bytes);
    let mut hash = [0u8; 16];
    bi.read_exact(&mut hash).unwrap();
    bi.read_exact(&mut buf4).unwrap();
    let n_blocks = u32::from_be_bytes(buf4);
    let mut block_metas = Vec::new();
    for _ in 0..n_blocks {
        bi.read_exact(&mut buf4).unwrap();
        let u_size = u32::from_be_bytes(buf4);
        bi.read_exact(&mut buf4).unwrap();
        let c_size = u32::from_be_bytes(buf4);
        let mut f2 = [0u8; 2];
        bi.read_exact(&mut f2).unwrap();
        let blk_flags = u16::from_be_bytes(f2);
        block_metas.push((u_size, c_size, blk_flags));
    }

    let mut blocks_start = if flags & 0x80 != 0 {
        header_end
    } else {
        header_end + compressed_blockinfo
    };
    if flags & 0x200 != 0 {
        let pad = (16 - (blocks_start % 16)) % 16;
        blocks_start += pad;
    }
    let mut cursor_data = blocks_start;
    let mut decomp = Vec::new();
    for (u_size, c_size, blk_flags) in &block_metas {
        let comp_chunk = &data[cursor_data..cursor_data + *c_size as usize];
        cursor_data += *c_size as usize;
        let ctype = blk_flags & 0x3f;
        let block_out: Vec<u8> = match ctype {
            0 => comp_chunk.to_vec(),
            2 | 3 => lz4::decompress(comp_chunk, *u_size as usize).unwrap(),
            _ => panic!("unknown block compression {}", ctype),
        };
        decomp.extend_from_slice(&block_out);
    }

    bi.read_exact(&mut buf4).unwrap();
    let n_dirs = u32::from_be_bytes(buf4);
    for i in 0..n_dirs {
        bi.read_exact(&mut buf8).unwrap();
        let off = u64::from_be_bytes(buf8) as usize;
        bi.read_exact(&mut buf8).unwrap();
        let size = u64::from_be_bytes(buf8) as usize;
        bi.read_exact(&mut buf4).unwrap();
        let _flags = u32::from_be_bytes(buf4);
        let name = read_cstring(&mut bi);
        let safe = format!("{:02}_{}", i, name.replace('/', "_"));
        let out_path = format!("{}/{}", out_dir, safe);
        fs::write(&out_path, &decomp[off..off + size]).unwrap();
        println!("wrote {} ({} bytes) -> {}", name, size, out_path);
    }
}
