fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../assets/logos/kuroya.ico");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let mut resource = winresource::WindowsResource::new();
    resource
        .set_icon("../../assets/logos/kuroya.ico")
        .set("FileDescription", "Kuroya")
        .set("ProductName", "Kuroya")
        .set("CompanyName", "Kuroya Contributors")
        .set("InternalName", "kuroya")
        .set("OriginalFilename", "kuroya.exe")
        .set("LegalCopyright", "Copyright 2026 Kuroya Contributors")
        .set_version_info(
            winresource::VersionInfo::FILEVERSION,
            cargo_version_number(),
        )
        .set_version_info(
            winresource::VersionInfo::PRODUCTVERSION,
            cargo_version_number(),
        )
        .set_manifest(WINDOWS_APP_MANIFEST);

    resource
        .compile()
        .expect("compile Kuroya Windows application resources");
}

fn cargo_version_number() -> u64 {
    let major = cargo_version_part("CARGO_PKG_VERSION_MAJOR");
    let minor = cargo_version_part("CARGO_PKG_VERSION_MINOR");
    let patch = cargo_version_part("CARGO_PKG_VERSION_PATCH");
    (major << 48) | (minor << 32) | (patch << 16)
}

fn cargo_version_part(name: &str) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .map(u64::from)
        .unwrap_or(0)
}

const WINDOWS_APP_MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity version="1.0.0.0" processorArchitecture="*" name="Kuroya.Kuroya" type="win32" />
  <description>Kuroya</description>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false" />
      </requestedPrivileges>
    </security>
  </trustInfo>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true/pm</dpiAware>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2, PerMonitor</dpiAwareness>
      <longPathAware xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">true</longPathAware>
    </windowsSettings>
  </application>
  <dependency>
    <dependentAssembly>
      <assemblyIdentity
        type="win32"
        name="Microsoft.Windows.Common-Controls"
        version="6.0.0.0"
        processorArchitecture="*"
        publicKeyToken="6595b64144ccf1df"
        language="*" />
    </dependentAssembly>
  </dependency>
</assembly>
"#;
