fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("icons/icon.ico");
        res.set("ProductName", "串口调试助手");
        res.set("FileDescription", "串口调试助手");
        res.set("CompanyName", "boast");
        res.compile().expect("Failed to compile Windows resources");
    }
}
