#[test]
fn wgpu_native_backend_features_are_enabled() {
    let backends = wgpu::Instance::enabled_backend_features();
    assert!(
        !backends.is_empty(),
        "wgpu must compile with a native backend or eframe panics at startup"
    );

    #[cfg(target_os = "windows")]
    assert!(
        backends.intersects(wgpu::Backends::DX12 | wgpu::Backends::VULKAN | wgpu::Backends::GL)
    );

    #[cfg(target_os = "macos")]
    assert!(backends.intersects(wgpu::Backends::METAL | wgpu::Backends::VULKAN));

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    assert!(backends.intersects(wgpu::Backends::VULKAN | wgpu::Backends::GL));
}
