fn main() {
    // Compila a interface gráfica do Slint
    slint_build::compile("ui/appwindow.slint").expect("Falha ao compilar a interface Slint");

    #[cfg(windows)]
    {
        // Incorpora os metadados do executável e o ícone nativo nos recursos do Windows
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("ProductName", "Voicemeeter Auto Restart (VBAR)");
        res.set("FileDescription", "Voicemeeter Auto Restart (VBAR)");
        res.set("InternalName", "VBAR");
        res.set("OriginalFilename", "VBAR.exe");
        res.set("LegalCopyright", "Copyright (C) 2026");
        let _ = res.compile();
    }
}
