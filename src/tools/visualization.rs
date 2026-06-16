use crate::tools::{AgentTool, ToolContext};
use miette::Result;
use ollama_rs::generation::tools::{ToolFunctionInfo, ToolInfo, ToolType};
use plotters::prelude::*;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use std::path::Path;

#[derive(Deserialize, Debug, JsonSchema)]
struct SeriesData {
    name: String,
    x: Vec<f64>,
    y: Vec<f64>,
}

#[derive(Deserialize, Debug, JsonSchema)]
struct GenerateGraphArgs {
    /// "line" or "scatter"
    chart_type: String,
    title: String,
    x_label: Option<String>,
    y_label: Option<String>,
    /// Where to save the generated PNG (e.g., 'chart.png')
    output_path: String,
    series: Vec<SeriesData>,
}

pub struct GenerateGraphTool;

#[async_trait::async_trait]
impl AgentTool for GenerateGraphTool {
    fn name(&self) -> &'static str {
        "generate_graph"
    }

    fn description(&self) -> &'static str {
        "Generates a PNG graph/chart (line or scatter) from structured numeric data and saves it to a specified output_path. \
        Provide 'chart_type' ('line' or 'scatter'), 'title', 'output_path' (e.g. 'chart.png'), and 'series' (array of objects with 'name', 'x', 'y' arrays)."
    }

    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings
            .into_generator()
            .into_root_schema_for::<GenerateGraphArgs>();

        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload,
            },
        }
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let parsed_args: GenerateGraphArgs = serde_json::from_value(args.clone())
            .map_err(|e| miette::miette!("Failed to parse arguments: {}", e))?;

        if parsed_args.series.is_empty() {
            return Err(miette::miette!("No data series provided."));
        }

        let mut min_x = f64::MAX;
        let mut max_x = f64::MIN;
        let mut min_y = f64::MAX;
        let mut max_y = f64::MIN;

        for s in &parsed_args.series {
            for &x in &s.x {
                if x < min_x {
                    min_x = x;
                }
                if x > max_x {
                    max_x = x;
                }
            }
            for &y in &s.y {
                if y < min_y {
                    min_y = y;
                }
                if y > max_y {
                    max_y = y;
                }
            }
        }

        if min_x > max_x || min_y > max_y {
            min_x = 0.0;
            max_x = 1.0;
            min_y = 0.0;
            max_y = 1.0;
        } else {
            let range_x_pad = (max_x - min_x) * 0.1;
            let range_y_pad = (max_y - min_y) * 0.1;
            min_x -= range_x_pad.max(0.01);
            max_x += range_x_pad.max(0.01);
            min_y -= range_y_pad.max(0.01);
            max_y += range_y_pad.max(0.01);
        }

        let output_path = Path::new(&parsed_args.output_path);

        let root = BitMapBackend::new(&output_path, (800, 600)).into_drawing_area();
        root.fill(&WHITE)
            .map_err(|e| miette::miette!("Drawing error: {}", e))?;

        let mut chart = ChartBuilder::on(&root)
            .caption(&parsed_args.title, ("sans-serif", 40).into_font())
            .margin(10)
            .x_label_area_size(40)
            .y_label_area_size(50)
            .build_cartesian_2d(min_x..max_x, min_y..max_y)
            .map_err(|e| miette::miette!("Chart creation error: {}", e))?;

        let mut mesh = chart.configure_mesh();
        if let Some(lbl) = &parsed_args.x_label {
            mesh.x_desc(lbl);
        }
        if let Some(lbl) = &parsed_args.y_label {
            mesh.y_desc(lbl);
        }
        mesh.draw()
            .map_err(|e| miette::miette!("Mesh draw error: {}", e))?;

        let colors = [&RED, &BLUE, &GREEN, &MAGENTA, &CYAN, &YELLOW];

        for (i, series) in parsed_args.series.iter().enumerate() {
            if series.x.len() != series.y.len() {
                return Err(miette::miette!(
                    "Series '{}' x and y arrays must have the same length",
                    series.name
                ));
            }

            let color = *colors[i % colors.len()];
            let data_points: Vec<(f64, f64)> = series
                .x
                .iter()
                .copied()
                .zip(series.y.iter().copied())
                .collect();

            if parsed_args.chart_type.to_lowercase() == "scatter" {
                chart
                    .draw_series(
                        data_points
                            .iter()
                            .map(|point| Circle::new(*point, 5, color.filled())),
                    )
                    .map_err(|e| miette::miette!("Draw error: {}", e))?
                    .label(&series.name)
                    .legend(move |(x, y)| Circle::new((x, y), 5, color.filled()));
            } else {
                chart
                    .draw_series(LineSeries::new(data_points, color))
                    .map_err(|e| miette::miette!("Draw error: {}", e))?
                    .label(&series.name)
                    .legend(move |(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], color));
            }
        }

        chart
            .configure_series_labels()
            .background_style(WHITE.mix(0.8))
            .border_style(BLACK)
            .draw()
            .map_err(|e| miette::miette!("Legend draw error: {}", e))?;

        root.present()
            .map_err(|e| miette::miette!("File save error: {}", e))?;

        let abs_path =
            std::fs::canonicalize(output_path).unwrap_or_else(|_| output_path.to_path_buf());
        Ok(format!(
            "Successfully generated chart and saved to: {}",
            abs_path.display()
        ))
    }
}
