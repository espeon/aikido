fn main() {
    let builder = kidomc_lib::create_builder();
    builder
        .export(
            specta_typescript::Typescript::default(),
            "src/bindings.ts",
        )
        .expect("Failed to export typescript bindings");
    println!("bindings.ts generated");
}
