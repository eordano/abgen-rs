
#[cfg(not(feature = "gpu"))]
pub fn is_gpu_available() -> bool {
    false
}

#[cfg(feature = "gpu")]
pub mod bc7_mode6;

#[cfg(feature = "gpu")]
use std::sync::OnceLock;

#[cfg(feature = "gpu")]
pub struct GpuContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter_name: String,
    pub bc7_mode6_pipeline: wgpu::ComputePipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
}

#[cfg(feature = "gpu")]
static GPU_CTX: OnceLock<Option<GpuContext>> = OnceLock::new();

#[cfg(feature = "gpu")]
pub fn detect() -> Option<&'static GpuContext> {
    GPU_CTX.get_or_init(|| init_gpu().ok()).as_ref()
}

#[cfg(feature = "gpu")]
pub fn is_gpu_available() -> bool {
    detect().is_some()
}

#[cfg(feature = "gpu")]
fn init_gpu() -> anyhow::Result<GpuContext> {
    use wgpu::{
        Backends, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, BufferBindingType,
        ComputePipelineDescriptor, DeviceDescriptor, Features, Instance, InstanceDescriptor, Limits,
        PipelineLayoutDescriptor, PowerPreference, RequestAdapterOptions, ShaderModuleDescriptor,
        ShaderSource, ShaderStages,
    };

    let instance = Instance::new(InstanceDescriptor {
        backends: Backends::VULKAN | Backends::METAL | Backends::DX12,
        ..Default::default()
    });

    let adapter = pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
        power_preference: PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .ok_or_else(|| anyhow::anyhow!("no GPU adapter (vulkan/metal/dx12)"))?;

    let info = adapter.get_info();
    let adapter_name = format!("{} ({:?})", info.name, info.backend);

    let (device, queue) = pollster::block_on(adapter.request_device(
        &DeviceDescriptor {
            label: Some("abgen-bc7-gpu"),
            required_features: Features::empty(),
            required_limits: Limits::downlevel_defaults(),
            memory_hints: wgpu::MemoryHints::Performance,
        },
        None,
    ))?;

    let shader_src = include_str!("../../shaders/bc7_mode6.wgsl");
    let module = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("bc7_mode6.wgsl"),
        source: ShaderSource::Wgsl(shader_src.into()),
    });

    let bgl = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("bc7_mode6_bgl"),
        entries: &[
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 2,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });

    let pl = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("bc7_mode6_pl"),
        bind_group_layouts: &[&bgl],
        push_constant_ranges: &[],
    });

    let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
        label: Some("bc7_mode6_pipeline"),
        layout: Some(&pl),
        module: &module,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });

    Ok(GpuContext {
        device,
        queue,
        adapter_name,
        bc7_mode6_pipeline: pipeline,
        bind_group_layout: bgl,
    })
}
