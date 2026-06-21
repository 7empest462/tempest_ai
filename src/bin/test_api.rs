use schemars::JsonSchema;

#[derive(JsonSchema)]
pub struct SystemInfoArgs {}

pub fn get_schema() -> schemars::Schema {
    let mut settings = schemars::generate::SchemaSettings::draft07();
    settings.inline_subschemas = true;
    let generator = settings.into_generator();

    generator.into_root_schema_for::<SystemInfoArgs>()
}

fn main() {
    let schema = get_schema();
    println!("{}", serde_json::to_string_pretty(&schema).unwrap());
}
