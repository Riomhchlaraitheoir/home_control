use slint_build::CompilerConfiguration;

fn main() {
    let config = CompilerConfiguration::new()
        .with_style("material".to_string())
        .with_library_paths(
            std::collections::HashMap::from([(
                "material".to_string(),
                std::path::Path::new(&std::env::var_os("CARGO_MANIFEST_DIR").unwrap())
                    .join("material-1.0/material.slint"),
            )]),
        );
    slint_build::compile_with_config("src/ui/main.slint", config).expect("Slint build failed");
}
