use log::debug;
use std::borrow::Cow;
use std::convert::TryInto;
use wgpu::util::DeviceExt;

async fn run() {}

struct GPU {
    device: wgpu::Device,
    queue: wgpu::Queue,
    adapter: wgpu::Adapter,
}

impl GPU {
    async fn new() -> GPU {
        let backend = if let Ok(backend) = std::env::var("WGPU_BACKEND") {
            match backend.to_lowercase().as_str() {
                "vulkan" => wgpu::BackendBit::VULKAN,
                "metal" => wgpu::BackendBit::METAL,
                "dx12" => wgpu::BackendBit::DX12,
                "dx11" => wgpu::BackendBit::DX11,
                "gl" => wgpu::BackendBit::GL,
                "webgpu" => wgpu::BackendBit::BROWSER_WEBGPU,
                other => panic!("Unknown backend: {}", other),
            }
        } else {
            wgpu::BackendBit::PRIMARY
        };

        let power_preference = if let Ok(power_preference) = std::env::var("WGPU_POWER_PREF") {
            match power_preference.to_lowercase().as_str() {
                "low" => wgpu::PowerPreference::LowPower,
                "high" => wgpu::PowerPreference::HighPerformance,
                other => panic!("Unknown power preference: {}", other),
            }
        } else {
            wgpu::PowerPreference::default()
        };

        let instance = wgpu::Instance::new(backend);

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference,
                compatible_surface: None,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .unwrap();

        GPU {
            device,
            queue,
            adapter,
        }
    }

    fn create_shader(&self, source: &str) -> wgpu::ShaderModule {
        let mut flags = wgpu::ShaderFlags::VALIDATION;
        match self.adapter.get_info().backend {
            wgpu::Backend::Vulkan | wgpu::Backend::Metal | wgpu::Backend::Gl => {
                flags |= wgpu::ShaderFlags::EXPERIMENTAL_TRANSLATION;
            }
            _ => {}
        }

        self.device
            .create_shader_module(&wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(source)),
                flags,
            })
    }

    async fn do_thing(
        &self,
        module: &wgpu::ShaderModule,
        data: &[u8],
        workgroups: (u32, u32, u32),
    ) -> Result<wgpu::Buffer, wgpu::BufferAsyncError> {
        let size = data.len() as wgpu::BufferAddress;

        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size,
            usage: wgpu::BufferUsage::MAP_READ | wgpu::BufferUsage::COPY_DST,
            mapped_at_creation: false,
        });

        let storage_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Storage Buffer"),
                contents: data,
                usage: wgpu::BufferUsage::STORAGE
                    | wgpu::BufferUsage::COPY_DST
                    | wgpu::BufferUsage::COPY_SRC,
            });

        let compute_pipeline =
            self.device
                .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: None,
                    layout: None,
                    module,
                    entry_point: "main",
                });

        let bind_group_layout = compute_pipeline.get_bind_group_layout(0);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: storage_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: storage_buffer.as_entire_binding(),
                },
            ],
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut cpass =
                encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });
            cpass.set_pipeline(&compute_pipeline);
            cpass.set_bind_group(0, &bind_group, &[]);
            cpass.insert_debug_marker("TODO");
            let (x, y, z) = workgroups;
            cpass.dispatch(x, y, z);
        }
        encoder.copy_buffer_to_buffer(&storage_buffer, 0, &staging_buffer, 0, size);

        debug!("submit");
        self.queue.submit(Some(encoder.finish()));

        debug!("slice");
        let buffer_slice = staging_buffer.slice(..); // What's this for?
        debug!("slice future");
        let buffer_future = buffer_slice.map_async(wgpu::MapMode::Read);

        debug!("poll");
        self.device.poll(wgpu::Maintain::Wait);

        debug!("slice future wait");
        buffer_future.await?;
        debug!("done");

        Ok(staging_buffer)
    }

    fn drop_buffer(&self, buffer: wgpu::Buffer) {
        buffer.unmap();
    }
}

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        env_logger::init();
        pollster::block_on(run());
    }
    #[cfg(target_arch = "wasm32")]
    {
        std::panic::set_hook(Box::new(console_error_panic_hook::hook));
        console_log::init().expect("could not initialize logger");
        wasm_bindgen_futures::spawn_local(run());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn async_qround() {
        let gpu = GPU::new().await;
        let chacha_shader = gpu.create_shader(include_str!("chacha20_qround.wgsl"));

        let state: Vec<u32> = vec![
            0x879531e0, 0xc5ecf37d, 0x516461b1, 0xc9a62f8a, // 0-3
            0x44c20ef3, 0x3390af7f, 0xd9fc690b, 0x2a5f714c, // 4-7
            0x53372767, 0xb00a5631, 0x974c541a, 0x359e9963, // 8-11
            0x5c971061, 0x3d631689, 0x2098d9d6, 0x91dbd320, // 12-15
        ];

        let buffer = gpu
            .do_thing(&chacha_shader, bytemuck::cast_slice(&state), (1, 1, 1))
            .await
            .unwrap();

        let data = buffer.slice(..).get_mapped_range();
        let result: Vec<u32> = data
            .chunks_exact(4)
            .map(|b| u32::from_ne_bytes(b.try_into().unwrap()))
            .collect();

        drop(data);
        gpu.drop_buffer(buffer);

        assert_eq!(
            result,
            vec![
                0x879531e0, 0xc5ecf37d, 0xbdb886dc, 0xc9a62f8a, // 0-3
                0x44c20ef3, 0x3390af7f, 0xd9fc690b, 0xcfacafd2, // 4-7
                0xe46bea80, 0xb00a5631, 0x974c541a, 0x359e9963, // 8-11
                0x5c971061, 0xccc07c79, 0x2098d9d6, 0x91dbd320, // 12-15
            ]
        )
    }

    #[test]
    fn qround() {
        pollster::block_on(async_qround());
    }

    async fn async_block() {
        let gpu = GPU::new().await;
        let chacha_shader = gpu.create_shader(include_str!("chacha20_block.wgsl"));

        let routine: Vec<u32> = vec![
            0x0, 0x0, 0x0, 0x0, // 0-3
            0x03020100, 0x07060504, 0x0b0a0908, 0x0f0e0d0c, // 4-7
            0x13121110, 0x17161514, 0x1b1a1918, 0x1f1e1d1c, // 8-11
            0x00000001, 0x09000000, 0x4a000000, 0x00000000, // 12-15
        ];
        let buffer = gpu
            .do_thing(&chacha_shader, bytemuck::cast_slice(&routine), (1, 1, 1))
            .await
            .unwrap();

        let data = buffer.slice(..).get_mapped_range();
        let result: Vec<u32> = data
            .chunks_exact(4)
            .map(|b| u32::from_ne_bytes(b.try_into().unwrap()))
            .collect();

        drop(data);
        gpu.drop_buffer(buffer);

        assert_eq!(
            result,
            vec![
                0x837778ab, 0xe238d763, 0xa67ae21e, 0x5950bb2f, // 0-3
                0xc4f2d0c7, 0xfc62bb2f, 0x8fa018fc, 0x3f5ec7b7, // 4-7
                0x335271c2, 0xf29489f3, 0xeabda8fc, 0x82e46ebd, // 8-11
                0xd19c12b4, 0xb04e16de, 0x9e83d0cb, 0x4e3c50a2, // 12-15
            ]
        )
    }

    #[test]
    fn block() {
        pollster::block_on(async_block());
    }
}
