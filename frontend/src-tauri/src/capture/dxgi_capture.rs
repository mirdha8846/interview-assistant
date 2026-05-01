use windows::core::{Result, Interface, ComInterface};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, IDXGIFactory1, IDXGIAdapter1, IDXGIOutput1, IDXGIOutputDuplication,
    DXGI_ERROR_WAIT_TIMEOUT, IDXGIResource, IDXGIOutput,
};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
    D3D11_SDK_VERSION, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING, D3D11_CPU_ACCESS_READ, D3D11_MAP_READ,
    D3D11_MAPPED_SUBRESOURCE,
};
use windows::Win32::Graphics::Direct3D::{D3D_FEATURE_LEVEL_11_0, D3D_DRIVER_TYPE, D3D_DRIVER_TYPE_UNKNOWN};
use std::ptr;

pub struct DxgiCapture {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    duplication: IDXGIOutputDuplication,
}

impl DxgiCapture {
    pub fn new() -> Result<Self> {
        unsafe {
            let factory: IDXGIFactory1 = CreateDXGIFactory1()?;
            let adapter: IDXGIAdapter1 = factory.EnumAdapters1(0)?;
            let output: IDXGIOutput = adapter.EnumOutputs(0)?;
            let output1: IDXGIOutput1 = output.cast()?;

            let mut device = None;
            let mut context = None;
            
            D3D11CreateDevice(
                &adapter,
                D3D_DRIVER_TYPE_UNKNOWN,
                None,
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(&[D3D_FEATURE_LEVEL_11_0]),
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )?;

            let device = device.unwrap();
            let context = context.unwrap();
            let duplication = output1.DuplicateOutput(&device)?;

            Ok(Self {
                device,
                context,
                duplication,
            })
        }
    }

    pub fn capture_frame(&self) -> Result<Vec<u8>> {
        unsafe {
            let mut resource = None;
            let mut info = Default::default();
            
            // Acquire frame
            match self.duplication.AcquireNextFrame(100, &mut info, &mut resource) {
                Ok(_) => (),
                Err(e) => {
                    if e.code() == DXGI_ERROR_WAIT_TIMEOUT {
                        return Err(e);
                    }
                    return Err(e);
                }
            };

            let resource = resource.unwrap();
            let texture: ID3D11Texture2D = resource.cast()?;
            
            // Create staging texture to read from CPU
            let mut desc = D3D11_TEXTURE2D_DESC::default();
            texture.GetDesc(&mut desc);
            
            desc.Usage = D3D11_USAGE_STAGING;
            desc.BindFlags = 0;
            desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
            desc.MiscFlags = 0;

            let mut staging_texture = None;
            self.device.CreateTexture2D(&desc, None, Some(&mut staging_texture))?;
            let staging_texture = staging_texture.unwrap();

            self.context.CopyResource(&staging_texture, &texture);
            
            // Map and read
            let mut mapped_resource = D3D11_MAPPED_SUBRESOURCE::default();
            self.context.Map(&staging_texture, 0, D3D11_MAP_READ, 0, Some(&mut mapped_resource))?;
            
            // Copy data
            let size = (desc.Width * desc.Height * 4) as usize;
            let mut buffer = vec![0u8; size];
            
            ptr::copy_nonoverlapping(mapped_resource.pData as *const u8, buffer.as_mut_ptr(), size);
            
            self.context.Unmap(&staging_texture, 0);
            self.duplication.ReleaseFrame()?;

            Ok(buffer)
        }
    }
}
