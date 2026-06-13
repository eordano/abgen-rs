
use super::GpuContext;
use wgpu::util::DeviceExt;

const PIXELS_PER_BLOCK: usize = 16;
const BYTES_PER_INPUT_BLOCK: usize = PIXELS_PER_BLOCK * 4;
const BYTES_PER_OUTPUT_BLOCK: usize = 16;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Zeroable, bytemuck::Pod)]
struct Params {
    num_blocks: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

pub fn encode_mode6(ctx: &GpuContext, rgba_block_major: &[u8]) -> Vec<u8> {
    assert!(rgba_block_major.len() % BYTES_PER_INPUT_BLOCK == 0);
    let num_blocks = rgba_block_major.len() / BYTES_PER_INPUT_BLOCK;
    if num_blocks == 0 {
        return Vec::new();
    }

    let device = &ctx.device;
    let queue = &ctx.queue;

    let input_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("bc7_in"),
        contents: rgba_block_major,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });

    let out_bytes = (num_blocks * BYTES_PER_OUTPUT_BLOCK) as u64;
    let output_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("bc7_out"),
        size: out_bytes,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    let readback_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("bc7_readback"),
        size: out_bytes,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let params = Params {
        num_blocks: num_blocks as u32,
        _pad0: 0,
        _pad1: 0,
        _pad2: 0,
    };
    let params_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("bc7_params"),
        contents: bytemuck::bytes_of(&params),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("bc7_bg"),
        layout: &ctx.bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: input_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: output_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: params_buf.as_entire_binding(),
            },
        ],
    });

    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("bc7_enc"),
    });
    {
        let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("bc7_pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&ctx.bc7_mode6_pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(num_blocks as u32, 1, 1);
    }
    enc.copy_buffer_to_buffer(&output_buf, 0, &readback_buf, 0, out_bytes);
    queue.submit(Some(enc.finish()));

    let slice = readback_buf.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    device.poll(wgpu::Maintain::Wait);
    rx.recv()
        .expect("readback channel closed")
        .expect("buffer map failed");
    let data = slice.get_mapped_range();
    let out = data.to_vec();
    drop(data);
    readback_buf.unmap();
    out
}
