use wgpu::{util::DeviceExt, BufferUsages};

// Get a device and a queue, honoring WGPU_ADAPTER_NAME and WGPU_BACKEND environment variables
pub async fn request_device_queue() -> (wgpu::Device, wgpu::Queue) {
    let instance = wgpu::Instance::new(wgpu::Backends::all());
    let backends = wgpu::util::backend_bits_from_env().unwrap_or_else(wgpu::Backends::all);
    let adapter = wgpu::util::initialize_adapter_from_env_or_default(&instance, backends, None)
        .await
        .expect("No GPU found given preference");

    // `request_device` instantiates the feature specific connection to the GPU, defining some parameters,
    //  `features` being the available features.
    adapter
        .request_device(&wgpu::DeviceDescriptor::default(), None)
        .await
        .expect("Could not create adapter for GPU device")
}

pub fn create_buffer_init<T: Clone + bytemuck::Pod>(
    device: &wgpu::Device,
    array: &[T],
    name: &str,
    usage: BufferUsages,
) -> wgpu::Buffer {
    let array = resize(array.to_vec());

    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(name),
        contents: bytemuck::cast_slice(&array),
        usage,
    })
}

pub fn buffer(
    device: &wgpu::Device,
    requested_size_bytes: usize,
    name: &str,
    usage: BufferUsages,
) -> wgpu::Buffer {
    let slice_size = usize::max(16, requested_size_bytes);
    let size = slice_size as wgpu::BufferAddress;
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(name),
        size,
        mapped_at_creation: false,
        usage,
    })
}

pub fn resize<T: Clone + bytemuck::Pod>(mut array: Vec<T>) -> Vec<T> {
    let size = array.len();
    if size < 4 && size != 0 {
        array.resize(size + 4 - size % 4, T::zeroed());
    }

    array
}

// Padding as byte
pub fn padding(data: &[u8], chunk_size: usize, padding_size: usize) -> Vec<u8> {
    let mut padded_data = vec![];
    let n = data.len() / chunk_size;
    for i in 0..n {
        padded_data.extend(&data[chunk_size * i..chunk_size * (i + 1)]);
        padded_data.extend(vec![0; padding_size]);
    }
    padded_data
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_request_device_queue() {
        pollster::block_on(crate::resource::request_device_queue());
    }

    #[test]
    fn test_create_buffer_init() {
        let (device, _) = pollster::block_on(crate::resource::request_device_queue());
        let data = [1.0, 2.0, 3.0, 4.0];
        let _ = crate::resource::create_buffer_init(
            &device,
            &data,
            "test",
            wgpu::BufferUsages::STORAGE,
        );
    }
}
