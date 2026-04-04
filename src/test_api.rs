use schemars::JsonSchema;

#[derive(JsonSchema)]
pub struct SystemInfoArgs {}

pub fn get_schema() -> schemars::Schema {
    let mut settings = schemars::generate::SchemaSettings::draft07();
    settings.inline_subschemas = true;
    let mut generator = settings.into_generator();
    let root = generator.into_root_schema_for::<SystemInfoArgs>();
    root.into()
}
